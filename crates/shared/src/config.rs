//! Application configuration.
//!
//! `[server]` and `[telemetry]` are common to all services, and each of them
//! claims a section named after itself with [`AppConfig::section`], ignoring
//! the rest.
//!
//! A section merges four layers, lowest precedence first:
//!
//! 1. A TOML file, `./config.toml` or the path in `QUIZ_ARENA_CONFIG_PATH`
//! 2. Inline TOML in `QUIZ_ARENA_CONFIG`
//! 3. Alias variables a section declares per field, e.g.
//!    `OTEL_EXPORTER_OTLP_ENDPOINT` for `[telemetry]`
//! 4. `QUIZ_ARENA_<SECTION>_<FIELD>`
//!
//! A field no layer sets takes the default it declares with
//! `#[facet(default = ...)]`. A field without one is required, and startup
//! fails when it is missing.
//!
//! Layers 3 and 4 derive a variable name per field of the struct rather than
//! parsing variable names, which keeps underscores in section and field names
//! unambiguous. The fields are read off the struct's [`Facet`] shape, which is
//! also what the merged layers are deserialized through.

use std::{collections::HashMap, env, ffi::OsString, net::SocketAddr, path::Path, str::FromStr};

use anyhow::{Context as _, anyhow, bail};
use config::{Config, File, FileFormat, ValueKind};
use facet::{Def, Facet, Field, ScalarType, Shape, StructKind, Type, UserType};
use facet_value::{VObject, Value, from_value};

const ENV_PREFIX: &str = "QUIZ_ARENA_";
const CONFIG_PATH_VAR: &str = "QUIZ_ARENA_CONFIG_PATH";
const INLINE_CONFIG_VAR: &str = "QUIZ_ARENA_CONFIG";
const DEFAULT_CONFIG_PATH: &str = "config.toml";
const SERVER_SECTION: &str = "server";
const TELEMETRY_SECTION: &str = "telemetry";

/// Project-level settings from the `[server]` section.
#[derive(Debug, Facet)]
#[facet(deny_unknown_fields)]
pub struct ServerConfig {
    #[facet(default = SocketAddr::from(([0, 0, 0, 0], 8080)))]
    pub listen_addr: SocketAddr,
}

/// Telemetry export settings from the `[telemetry]` section.
#[derive(Debug, Facet)]
#[facet(deny_unknown_fields)]
pub struct TelemetryConfig {
    /// Whether telemetry is exported at all.
    #[facet(default = false)]
    pub enabled: bool,
    /// Base OTLP endpoint for every signal.
    ///
    /// `OTEL_EXPORTER_OTLP_ENDPOINT` overrides the file value.
    #[facet(default = "http://localhost:4317".to_owned())]
    pub otlp_endpoint: String,
    /// OTLP transport.
    ///
    /// `OTEL_EXPORTER_OTLP_PROTOCOL` overrides the file value.
    #[facet(default = OtlpProtocol::Grpc)]
    pub otlp_protocol: OtlpProtocol,
}

/// OTLP transport.
#[derive(Debug, Clone, Copy, Facet)]
#[repr(u8)]
pub enum OtlpProtocol {
    #[facet(rename = "grpc")]
    Grpc,
    #[facet(rename = "http/protobuf")]
    HttpProtobuf,
}

/// The merged configuration tree plus the environment snapshot used to resolve
/// per-field overrides.
pub struct AppConfig {
    tree: Config,
    env: HashMap<String, OsString>,
    /// Environment variable names claimed by extracted sections, mapped to the
    /// config path each resolves to. Used to detect ambiguous names.
    claimed: HashMap<String, String>,
}

impl AppConfig {
    fn new(tree: Config, env: HashMap<String, OsString>) -> Self {
        Self {
            tree,
            env,
            claimed: HashMap::new(),
        }
    }

    /// Loads the file and inline layers and snapshots the environment.
    pub fn load() -> anyhow::Result<Self> {
        let explicit_path = unicode_var(CONFIG_PATH_VAR)?;
        let path = explicit_path.as_deref().unwrap_or(DEFAULT_CONFIG_PATH);

        let present = Path::new(path)
            .try_exists()
            .with_context(|| format!("failed to check for configuration file `{path}`"))?;

        // A file that exists is always loaded, and an explicitly configured
        // path must exist. Only the default path may be absent, because
        // defaults plus environment can carry a deployment.
        let mut builder = Config::builder().add_source(
            File::new(path, FileFormat::Toml).required(explicit_path.is_some() || present),
        );
        if let Some(inline) = unicode_var(INLINE_CONFIG_VAR)? {
            builder = builder.add_source(File::from_str(&inline, FileFormat::Toml));
        }

        let tree = builder
            .build()
            .with_context(|| format!("failed to load configuration from `{path}`"))?;

        // The whole environment, not just `QUIZ_ARENA_*`, because sections may
        // declare standard alias variables like `OTEL_EXPORTER_OTLP_ENDPOINT`.
        let env = env::vars_os()
            .filter_map(|(name, value)| name.into_string().ok().map(|name| (name, value)))
            .filter(|(name, _)| {
                name.as_str() != CONFIG_PATH_VAR && name.as_str() != INLINE_CONFIG_VAR
            })
            .collect();

        Ok(Self::new(tree, env))
    }

    /// Extracts the `[server]` section.
    pub fn server(&mut self) -> anyhow::Result<ServerConfig> {
        self.section(SERVER_SECTION, &[])
    }

    /// Extracts the `[telemetry]` section.
    ///
    /// The standard OpenTelemetry variables override the file layers as
    /// aliases.
    pub fn telemetry(&mut self) -> anyhow::Result<TelemetryConfig> {
        self.section(
            TELEMETRY_SECTION,
            &[
                ("otlp_endpoint", "OTEL_EXPORTER_OTLP_ENDPOINT"),
                ("otlp_protocol", "OTEL_EXPORTER_OTLP_PROTOCOL"),
            ],
        )
    }

    /// Extracts the named section into `T`, resolving environment overrides.
    ///
    /// A field is required unless it declares a default with
    /// `#[facet(default = ...)]` or is an `Option`. A missing required field,
    /// an unknown field, or an invalid value aborts startup, so neither an
    /// omission nor a typo passes silently. A struct rejects unknown fields
    /// only when it says `#[facet(deny_unknown_fields)]`.
    ///
    /// `T` is a struct with named fields. A nested struct is a nested table,
    /// and a missing table counts as an empty one, so its fields fall to their
    /// defaults. Map fields have dynamic keys and cannot be overridden from the
    /// environment.
    ///
    /// `aliases` holds `(field path, variable name)` pairs, dots separating
    /// nested segments. They apply before the derived `QUIZ_ARENA_*` names,
    /// which overwrite them.
    pub fn section<T>(&mut self, name: &str, aliases: &[(&str, &str)]) -> anyhow::Result<T>
    where
        T: Facet<'static>,
    {
        let mut tree = match self.tree.get::<config::Value>(name) {
            Ok(value) => {
                if !matches!(value.kind, ValueKind::Table(_)) {
                    bail!("config section `{name}` must be a table");
                }
                to_facet_value(value)
            }
            Err(config::ConfigError::NotFound(_)) => VObject::new().into(),
            Err(error) => {
                return Err(error).with_context(|| format!("invalid config section `{name}`"));
            }
        };

        let mut leaves = Vec::new();
        collect_leaves(T::SHAPE, &mut Vec::new(), &mut leaves);

        // Named in the final error, since a value that parses for its kind
        // can still be one the field rejects, such as an unknown enum variant.
        let mut applied = Vec::new();
        for (field, alias) in aliases {
            let leaf = leaves
                .iter()
                .find(|leaf| leaf.path.join(".") == *field)
                .with_context(|| {
                    format!("alias `{alias}` targets unknown field `{field}` in section `{name}`")
                })?;
            if self.apply_env(&mut tree, alias, name, leaf)? {
                applied.push((*alias).to_owned());
            }
        }
        for leaf in &leaves {
            let derived = env_var_name(name, &leaf.path);
            if self.apply_env(&mut tree, &derived, name, leaf)? {
                applied.push(derived);
            }
        }

        add_missing_tables(T::SHAPE, &mut tree);
        if let Some(field) = missing_required_field(T::SHAPE, &tree) {
            bail!("invalid config section `{name}`: missing required field `{field}`");
        }
        from_value(tree)
            .map_err(|error| anyhow!("{error}"))
            .with_context(|| {
                if applied.is_empty() {
                    format!("invalid config section `{name}`")
                } else {
                    format!(
                        "invalid config section `{name}` after applying {}",
                        applied.join(", ")
                    )
                }
            })
    }

    /// Writes the value of environment variable `var` into `tree` at the
    /// leaf, and reports whether the variable was set.
    ///
    /// Claims the name either way, so an ambiguity is caught whether or not the
    /// variable happens to be set in this environment.
    fn apply_env(
        &mut self,
        tree: &mut Value,
        var: &str,
        section: &str,
        leaf: &Leaf,
    ) -> anyhow::Result<bool> {
        self.claim(var, section, &leaf.path)?;
        let Some(raw) = self.env.get(var) else {
            return Ok(false);
        };
        let raw = raw
            .to_str()
            .with_context(|| format!("environment variable `{var}` is not valid Unicode"))?;
        let value = parse_env_value(raw, leaf.kind)
            .with_context(|| format!("invalid value in environment variable `{var}`"))?;
        insert_at_path(tree, &leaf.path, value);
        Ok(true)
    }

    /// Records that `env_name` resolves to a field of `section`. Two distinct
    /// fields mapping to one variable name is ambiguous, so that aborts
    /// startup.
    fn claim(&mut self, env_name: &str, section: &str, path: &[&str]) -> anyhow::Result<()> {
        let target = if path.is_empty() {
            section.to_owned()
        } else {
            format!("{section}.{}", path.join("."))
        };
        let existing = self
            .claimed
            .entry(env_name.to_owned())
            .or_insert_with(|| target.clone());
        if *existing != target {
            bail!(
                "environment variable `{env_name}` is ambiguous, \
                 it maps to both `{existing}` and `{target}`"
            );
        }
        Ok(())
    }
}

/// Reads an environment variable that may be unset but must be Unicode.
fn unicode_var(name: &str) -> anyhow::Result<Option<String>> {
    match env::var(name) {
        Ok(value) => Ok(Some(value)),
        Err(env::VarError::NotPresent) => Ok(None),
        Err(error @ env::VarError::NotUnicode(_)) => Err(error)
            .with_context(|| format!("environment variable `{name}` is not valid Unicode")),
    }
}

/// A field an environment variable can set: where it sits in the section and
/// how its text is read.
struct Leaf {
    path: Vec<&'static str>,
    kind: LeafKind,
}

#[derive(Clone, Copy)]
enum LeafKind {
    /// Parsed as exactly this type, so a value out of its range is refused
    /// here, where the variable is known.
    Scalar(ScalarType),
    /// A JSON array.
    List,
    /// Taken literally. Strings, and everything that parses from one, such as
    /// an enum or a socket address.
    Text,
}

/// The fields of a struct with named fields, which is what a table maps to.
fn named_fields(shape: &Shape) -> Option<&'static [Field]> {
    match shape.ty {
        Type::User(UserType::Struct(structure)) if structure.kind == StructKind::Struct => {
            Some(structure.fields)
        }
        _ => None,
    }
}

/// Walks a shape down to the fields that hold a value rather than a table.
fn collect_leaves(shape: &'static Shape, prefix: &mut Vec<&'static str>, out: &mut Vec<Leaf>) {
    // An optional field is set the way its inner type is.
    if let Def::Option(option) = shape.def {
        return collect_leaves(option.t(), prefix, out);
    }
    if let Some(fields) = named_fields(shape) {
        for field in fields {
            prefix.push(field.effective_name());
            collect_leaves(field.shape(), prefix, out);
            prefix.pop();
        }
        return;
    }
    let kind = match (shape.scalar_type(), shape.def) {
        (Some(ScalarType::Unit), _) | (None, Def::Map(_)) => return,
        (Some(scalar), _) => LeafKind::Scalar(scalar),
        (None, Def::List(_) | Def::Array(_) | Def::Slice(_) | Def::Set(_)) => LeafKind::List,
        (None, _) => LeafKind::Text,
    };
    out.push(Leaf {
        path: prefix.clone(),
        kind,
    });
}

/// Gives every nested table the tree lacks an empty one, so a table nobody
/// wrote resolves field by field like the section itself does. An optional
/// table stays absent.
fn add_missing_tables(shape: &'static Shape, tree: &mut Value) {
    let (Some(fields), Some(table)) = (named_fields(shape), tree.as_object_mut()) else {
        return;
    };
    for field in fields {
        let (key, shape) = (field.effective_name(), field.shape());
        if named_fields(shape).is_none() {
            continue;
        }
        if !table.contains_key(key) {
            table.insert(key, VObject::new());
        }
        add_missing_tables(shape, &mut table[key]);
    }
}

/// The dotted path of the first field that has neither a value nor a declared
/// default.
///
/// The deserializer would not complain. It fills a missing field from its
/// type's `Default` whenever there is one, which turns a forgotten string into
/// an empty one. Only a default the field declares counts here.
fn missing_required_field(shape: &'static Shape, tree: &Value) -> Option<String> {
    let (fields, table) = (named_fields(shape)?, tree.as_object()?);
    fields.iter().find_map(|field| {
        let (key, shape) = (field.effective_name(), field.shape());
        match table.get(key) {
            Some(value) => missing_required_field(shape, value).map(|rest| format!("{key}.{rest}")),
            None if field.has_default() || matches!(shape.def, Def::Option(_)) => None,
            None => Some(key.to_owned()),
        }
    })
}

/// Converts a value of the merged file layers.
fn to_facet_value(value: config::Value) -> Value {
    match value.kind {
        ValueKind::Nil => Value::NULL,
        ValueKind::Boolean(boolean) => boolean.into(),
        ValueKind::I64(number) => number.into(),
        ValueKind::U64(number) => number.into(),
        // TOML integers are 64 bit, so the wider kinds only carry values that
        // fit. One that does not becomes a float and fails the field's type.
        ValueKind::I128(number) => match i64::try_from(number) {
            Ok(number) => number.into(),
            Err(_) => (number as f64).into(),
        },
        ValueKind::U128(number) => match u64::try_from(number) {
            Ok(number) => number.into(),
            Err(_) => (number as f64).into(),
        },
        ValueKind::Float(number) => number.into(),
        ValueKind::String(string) => string.into(),
        ValueKind::Table(table) => table
            .into_iter()
            .map(|(key, value)| (key, to_facet_value(value)))
            .collect(),
        ValueKind::Array(items) => items.into_iter().map(to_facet_value).collect(),
    }
}

/// `("my_module", ["retry", "max_attempts"])` becomes
/// `QUIZ_ARENA_MY_MODULE_RETRY_MAX_ATTEMPTS`.
fn env_var_name(section: &str, path: &[&str]) -> String {
    let mut name = format!("{ENV_PREFIX}{}", section.to_uppercase());
    for segment in path {
        name.push('_');
        name.push_str(&segment.to_uppercase());
    }
    name
}

/// Parses a raw environment string into the value its field expects.
fn parse_env_value(raw: &str, kind: LeafKind) -> anyhow::Result<Value> {
    const INTEGER: &str = "expected an integer in range";

    fn parse<N: FromStr + Into<Value>>(raw: &str, expected: &'static str) -> anyhow::Result<Value>
    where
        N::Err: std::error::Error + Send + Sync + 'static,
    {
        Ok(raw.parse::<N>().context(expected)?.into())
    }

    match kind {
        LeafKind::Scalar(ScalarType::Bool) => parse::<bool>(raw, "expected a boolean"),
        LeafKind::Scalar(ScalarType::U8) => parse::<u8>(raw, INTEGER),
        LeafKind::Scalar(ScalarType::U16) => parse::<u16>(raw, INTEGER),
        LeafKind::Scalar(ScalarType::U32) => parse::<u32>(raw, INTEGER),
        LeafKind::Scalar(ScalarType::U64 | ScalarType::USize) => parse::<u64>(raw, INTEGER),
        LeafKind::Scalar(ScalarType::I8) => parse::<i8>(raw, INTEGER),
        LeafKind::Scalar(ScalarType::I16) => parse::<i16>(raw, INTEGER),
        LeafKind::Scalar(ScalarType::I32) => parse::<i32>(raw, INTEGER),
        LeafKind::Scalar(ScalarType::I64 | ScalarType::ISize) => parse::<i64>(raw, INTEGER),
        LeafKind::Scalar(ScalarType::F32 | ScalarType::F64) => {
            parse::<f64>(raw, "expected a number")
        }
        LeafKind::List => {
            let list: Value = facet_json::from_str(raw)
                .map_err(|error| anyhow!("{error}"))
                .context("expected a JSON array")?;
            if list.as_array().is_none() {
                bail!("expected a JSON array");
            }
            Ok(list)
        }
        LeafKind::Scalar(_) | LeafKind::Text => Ok(raw.into()),
    }
}

/// Writes `value` at `path`, creating intermediate tables as needed.
fn insert_at_path(tree: &mut Value, path: &[&str], value: Value) {
    let Some((last, parents)) = path.split_last() else {
        *tree = value;
        return;
    };
    let mut node = tree;
    for &key in parents {
        let table = node.as_object_mut().expect("intermediate nodes are tables");
        // A file may have set a scalar where the struct nests a table. The
        // environment override wins, so replace it.
        if !table.get(key).is_some_and(Value::is_object) {
            table.insert(key, VObject::new());
        }
        node = &mut table[key];
    }
    node.as_object_mut()
        .expect("intermediate nodes are tables")
        .insert(*last, value);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq, Facet)]
    #[facet(deny_unknown_fields)]
    struct Retry {
        #[facet(default = 3)]
        max_attempts: i64,
    }

    #[derive(Debug, PartialEq, Facet)]
    #[facet(deny_unknown_fields)]
    struct MyModuleConfig {
        #[facet(default = "sqlite::memory:".to_owned())]
        database_url: String,
        #[facet(default = 1234)]
        port: u16,
        retry: Retry,
        note: Option<String>,
    }

    #[derive(Debug, PartialEq, Facet)]
    #[facet(deny_unknown_fields)]
    struct MyConfig {
        #[facet(default)]
        module_x: String,
    }

    /// Deliberately collides with [`MyConfig`] in the environment, `my_module`
    /// + `x` and `my` + `module_x` both derive `QUIZ_ARENA_MY_MODULE_X`.
    #[derive(Debug, PartialEq, Facet)]
    #[facet(deny_unknown_fields)]
    struct XOnly {
        #[facet(default)]
        x: String,
    }

    #[derive(Debug, PartialEq, Facet)]
    #[facet(deny_unknown_fields)]
    struct UnsignedConfig {
        #[facet(default)]
        value: u64,
    }

    fn app(file: &str, inline: Option<&str>, env: &[(&str, &str)]) -> AppConfig {
        let mut builder = Config::builder().add_source(File::from_str(file, FileFormat::Toml));
        if let Some(inline) = inline {
            builder = builder.add_source(File::from_str(inline, FileFormat::Toml));
        }
        AppConfig::new(
            builder.build().unwrap(),
            env.iter()
                .map(|(name, value)| (name.to_string(), OsString::from(value)))
                .collect(),
        )
    }

    #[test]
    fn missing_section_yields_defaults() {
        let config: MyModuleConfig = app("", None, &[]).section("my_module", &[]).unwrap();
        assert_eq!(
            config,
            MyModuleConfig {
                database_url: "sqlite::memory:".to_owned(),
                port: 1234,
                retry: Retry { max_attempts: 3 },
                note: None,
            }
        );
    }

    #[test]
    fn inline_overrides_file() {
        let config: MyModuleConfig = app(
            "[my_module]\nport = 9000",
            Some("[my_module]\nport = 9001"),
            &[],
        )
        .section("my_module", &[])
        .unwrap();
        assert_eq!(config.port, 9001);
    }

    #[test]
    fn env_overrides_inline() {
        let config: MyModuleConfig = app(
            "[my_module]\ndatabase_url = \"from-file\"",
            Some("[my_module]\ndatabase_url = \"from-inline\""),
            &[("QUIZ_ARENA_MY_MODULE_DATABASE_URL", "from-env")],
        )
        .section("my_module", &[])
        .unwrap();
        assert_eq!(config.database_url, "from-env");
    }

    #[test]
    fn env_reaches_typed_nested_and_optional_fields() {
        let config: MyModuleConfig = app(
            "",
            None,
            &[
                ("QUIZ_ARENA_MY_MODULE_PORT", "8081"),
                ("QUIZ_ARENA_MY_MODULE_RETRY_MAX_ATTEMPTS", "9"),
                ("QUIZ_ARENA_MY_MODULE_NOTE", "hello"),
            ],
        )
        .section("my_module", &[])
        .unwrap();
        assert_eq!(config.port, 8081);
        assert_eq!(config.retry.max_attempts, 9);
        assert_eq!(config.note.as_deref(), Some("hello"));
    }

    #[test]
    fn optional_strings_preserve_literal_env_values() {
        for raw in ["12345", "00123", "true", "false", "1.5", "", "null"] {
            for var in ["QUIZ_ARENA_MY_MODULE_NOTE", "STD_NOTE"] {
                let config: MyModuleConfig =
                    app("[my_module]\nnote = \"from-file\"", None, &[(var, raw)])
                        .section("my_module", &[("note", "STD_NOTE")])
                        .unwrap();
                assert_eq!(config.note.as_deref(), Some(raw));
            }
        }
    }

    #[test]
    fn optional_scalars_still_parse_and_validate_env_values() {
        #[derive(Debug, Facet)]
        struct OptionalScalars {
            enabled: Option<bool>,
            count: Option<u16>,
        }

        let config: OptionalScalars = app(
            "",
            None,
            &[
                ("QUIZ_ARENA_OPTIONAL_ENABLED", "true"),
                ("QUIZ_ARENA_OPTIONAL_COUNT", "123"),
            ],
        )
        .section("optional", &[])
        .unwrap();
        assert_eq!(config.enabled, Some(true));
        assert_eq!(config.count, Some(123));

        for raw in ["abc", "70000"] {
            let error = app("", None, &[("QUIZ_ARENA_OPTIONAL_COUNT", raw)])
                .section::<OptionalScalars>("optional", &[])
                .unwrap_err();
            assert!(format!("{error:#}").contains("QUIZ_ARENA_OPTIONAL_COUNT"));
        }
    }

    #[test]
    fn invalid_env_value_names_the_variable() {
        let error = app("", None, &[("QUIZ_ARENA_MY_MODULE_PORT", "abc")])
            .section::<MyModuleConfig>("my_module", &[])
            .unwrap_err();
        assert!(format!("{error:#}").contains("QUIZ_ARENA_MY_MODULE_PORT"));
    }

    #[test]
    fn out_of_range_env_value_names_the_variable() {
        let error = app("", None, &[("QUIZ_ARENA_MY_MODULE_PORT", "70000")])
            .section::<MyModuleConfig>("my_module", &[])
            .unwrap_err();
        assert!(format!("{error:#}").contains("QUIZ_ARENA_MY_MODULE_PORT"));
    }

    #[cfg(unix)]
    #[test]
    fn non_unicode_env_value_is_reported_without_panicking() {
        use std::os::unix::ffi::OsStringExt as _;

        let mut config = AppConfig::new(
            Config::builder().build().unwrap(),
            [(
                "QUIZ_ARENA_MY_MODULE_DATABASE_URL".to_owned(),
                OsString::from_vec(vec![0xff]),
            )]
            .into(),
        );
        let error = config
            .section::<MyModuleConfig>("my_module", &[])
            .unwrap_err();

        assert!(format!("{error:#}").contains("is not valid Unicode"));
    }

    #[test]
    fn env_accepts_unsigned_integers_above_i64_max() {
        let config: UnsignedConfig = app(
            "",
            None,
            &[("QUIZ_ARENA_UNSIGNED_VALUE", "18446744073709551615")],
        )
        .section("unsigned", &[])
        .unwrap();
        assert_eq!(config.value, u64::MAX);
    }

    #[test]
    fn unknown_field_is_rejected() {
        let error = app("[my_module]\ndatabse_url = \"typo\"", None, &[])
            .section::<MyModuleConfig>("my_module", &[])
            .unwrap_err();
        assert!(format!("{error:#}").contains("my_module"));
    }

    #[test]
    fn non_table_section_is_rejected() {
        let error = app("my_module = 5", None, &[])
            .section::<MyModuleConfig>("my_module", &[])
            .unwrap_err();
        assert!(format!("{error:#}").contains("must be a table"));
    }

    #[test]
    fn ambiguous_env_names_across_sections_are_rejected() {
        let mut app = app("", None, &[]);
        let _: MyConfig = app.section("my", &[]).unwrap();
        let error = app.section::<XOnly>("my_module", &[]).unwrap_err();
        assert!(format!("{error:#}").contains("ambiguous"));
    }

    /// No defaults, so both fields are required.
    #[derive(Debug, Facet)]
    #[facet(deny_unknown_fields)]
    struct RequiredConfig {
        issuer: String,
        protocol: OtlpProtocol,
    }

    #[test]
    fn missing_required_field_is_reported_by_name() {
        let error = app("[required]\nprotocol = \"grpc\"", None, &[])
            .section::<RequiredConfig>("required", &[])
            .unwrap_err();
        assert!(format!("{error:#}").contains("missing required field `issuer`"));

        let error = app("", None, &[])
            .section::<RequiredConfig>("required", &[])
            .unwrap_err();
        assert!(format!("{error:#}").contains("missing required field"));
    }

    #[derive(Debug, Facet)]
    struct Inner {
        token: String,
    }

    #[derive(Debug, Facet)]
    struct Outer {
        inner: Inner,
        #[facet(default)]
        tags: Vec<String>,
    }

    #[test]
    fn missing_required_field_in_an_absent_table_is_reported_with_its_path() {
        let error = app("", None, &[])
            .section::<Outer>("outer", &[])
            .unwrap_err();
        assert!(format!("{error:#}").contains("missing required field `inner.token`"));
    }

    #[test]
    fn env_sets_a_list_from_a_json_array() {
        let config: Outer = app(
            "[outer.inner]\ntoken = \"t\"",
            None,
            &[("QUIZ_ARENA_OUTER_TAGS", r#"["a", "b"]"#)],
        )
        .section("outer", &[])
        .unwrap();
        assert_eq!(config.tags, ["a", "b"]);
        assert_eq!(config.inner.token, "t");

        let error = app("", None, &[("QUIZ_ARENA_OUTER_TAGS", "a,b")])
            .section::<Outer>("outer", &[])
            .unwrap_err();
        assert!(format!("{error:#}").contains("QUIZ_ARENA_OUTER_TAGS"));
    }

    #[test]
    fn env_alone_satisfies_required_fields() {
        let config: RequiredConfig = app(
            "",
            None,
            &[
                ("QUIZ_ARENA_REQUIRED_ISSUER", "https://auth.example"),
                ("QUIZ_ARENA_REQUIRED_PROTOCOL", "http/protobuf"),
            ],
        )
        .section("required", &[])
        .unwrap();
        assert_eq!(config.issuer, "https://auth.example");
        assert!(matches!(config.protocol, OtlpProtocol::HttpProtobuf));
    }

    #[test]
    fn value_the_field_rejects_names_the_variables_applied() {
        let error = app(
            "[required]\nissuer = \"https://auth.example\"",
            None,
            &[("QUIZ_ARENA_REQUIRED_PROTOCOL", "carrier-pigeon")],
        )
        .section::<RequiredConfig>("required", &[])
        .unwrap_err();
        assert!(format!("{error:#}").contains("QUIZ_ARENA_REQUIRED_PROTOCOL"));
    }

    #[test]
    fn partial_section_leaves_omitted_fields_at_their_defaults() {
        let config: MyModuleConfig = app("[my_module]\nport = 9000", None, &[])
            .section("my_module", &[])
            .unwrap();
        assert_eq!(config.port, 9000);
        assert_eq!(config.database_url, "sqlite::memory:");
        assert_eq!(config.retry.max_attempts, 3);
    }

    /// A file that sets one telemetry field must leave the rest at their
    /// defaults rather than at whatever a partial deserialize would produce.
    #[test]
    fn telemetry_partial_section_keeps_other_defaults() {
        let config = app("[telemetry]\nenabled = true", None, &[])
            .telemetry()
            .unwrap();
        assert!(config.enabled);
        assert_eq!(config.otlp_endpoint, "http://localhost:4317");
    }

    #[test]
    fn alias_env_overrides_file() {
        let config: MyModuleConfig = app(
            "[my_module]\ndatabase_url = \"from-file\"",
            None,
            &[("STD_DATABASE_URL", "from-alias")],
        )
        .section("my_module", &[("database_url", "STD_DATABASE_URL")])
        .unwrap();
        assert_eq!(config.database_url, "from-alias");
    }

    #[test]
    fn derived_env_overrides_alias_env() {
        let config: MyModuleConfig = app(
            "",
            None,
            &[
                ("STD_DATABASE_URL", "from-alias"),
                ("QUIZ_ARENA_MY_MODULE_DATABASE_URL", "from-env"),
            ],
        )
        .section("my_module", &[("database_url", "STD_DATABASE_URL")])
        .unwrap();
        assert_eq!(config.database_url, "from-env");
    }

    #[test]
    fn alias_reaches_nested_fields() {
        let config: MyModuleConfig = app("", None, &[("STD_MAX_ATTEMPTS", "7")])
            .section("my_module", &[("retry.max_attempts", "STD_MAX_ATTEMPTS")])
            .unwrap();
        assert_eq!(config.retry.max_attempts, 7);
    }

    #[test]
    fn alias_for_unknown_field_is_rejected() {
        let error = app("", None, &[])
            .section::<MyModuleConfig>("my_module", &[("databse_url", "STD_URL")])
            .unwrap_err();
        assert!(format!("{error:#}").contains("unknown field"));
    }
}
