//! Application configuration.
//!
//! `[server]` and `[telemetry]` are common to all services, and each of them
//! claims a section named after itself with [`AppConfig::section`], ignoring
//! the rest.
//!
//! A section merges five layers, lowest precedence first:
//!
//! 1. The struct's `Default`, which every layer below overlays
//! 2. A TOML file, `./config.toml` or the path in `QUIZ_ARENA_CONFIG_PATH`
//! 3. Inline TOML in `QUIZ_ARENA_CONFIG`
//! 4. Alias variables a section declares per field, e.g.
//!    `OTEL_EXPORTER_OTLP_ENDPOINT` for `[telemetry]`
//! 5. `QUIZ_ARENA_<SECTION>_<FIELD>`
//!
//! Layers 4 and 5 derive a variable name per field of the struct rather than
//! parsing variable names, which keeps underscores in section and field names
//! unambiguous.

use std::{collections::HashMap, env, ffi::OsString, net::SocketAddr, path::Path, sync::Mutex};

use anyhow::{Context as _, bail};
use config::{Config, File, FileFormat};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;

const ENV_PREFIX: &str = "QUIZ_ARENA_";
const CONFIG_PATH_VAR: &str = "QUIZ_ARENA_CONFIG_PATH";
const INLINE_CONFIG_VAR: &str = "QUIZ_ARENA_CONFIG";
const DEFAULT_CONFIG_PATH: &str = "config.toml";
const SERVER_SECTION: &str = "server";
const TELEMETRY_SECTION: &str = "telemetry";

/// Project-level settings from the `[server]` section.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServerConfig {
    pub listen_addr: SocketAddr,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            listen_addr: SocketAddr::from(([0, 0, 0, 0], 8080)),
        }
    }
}

/// Telemetry export settings from the `[telemetry]` section.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TelemetryConfig {
    /// Whether telemetry is exported at all.
    pub enabled: bool,
    /// Base OTLP endpoint for every signal.
    ///
    /// `OTEL_EXPORTER_OTLP_ENDPOINT` overrides the file value.
    pub otlp_endpoint: String,
    /// OTLP transport.
    ///
    /// `OTEL_EXPORTER_OTLP_PROTOCOL` overrides the file value.
    pub otlp_protocol: OtlpProtocol,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            otlp_endpoint: "http://localhost:4317".to_owned(),
            otlp_protocol: OtlpProtocol::Grpc,
        }
    }
}

/// OTLP transport.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum OtlpProtocol {
    #[serde(rename = "grpc")]
    Grpc,
    #[serde(rename = "http/protobuf")]
    HttpProtobuf,
}

/// The merged configuration tree plus the environment snapshot used to resolve
/// per-field overrides.
pub struct AppConfig {
    tree: Config,
    env: HashMap<String, OsString>,
    /// Environment variable names claimed by extracted sections, mapped to the
    /// config path each resolves to. Used to detect ambiguous names.
    claimed: Mutex<HashMap<String, String>>,
}

impl AppConfig {
    fn new(tree: Config, env: HashMap<String, OsString>) -> Self {
        Self {
            tree,
            env,
            claimed: Mutex::new(HashMap::new()),
        }
    }

    /// Loads the file and inline layers and snapshots the environment.
    pub fn load() -> anyhow::Result<Self> {
        let (path, explicit) = match env::var(CONFIG_PATH_VAR) {
            Ok(path) => (path, true),
            Err(env::VarError::NotPresent) => (DEFAULT_CONFIG_PATH.to_owned(), false),
            Err(error @ env::VarError::NotUnicode(_)) => {
                return Err(error).with_context(|| {
                    format!("environment variable `{CONFIG_PATH_VAR}` is not valid Unicode")
                });
            }
        };

        let present = Path::new(&path)
            .try_exists()
            .with_context(|| format!("failed to check for configuration file `{path}`"))?;

        // A file that exists is always loaded, and an explicitly configured
        // path must exist. Only the default path may be absent, because
        // defaults plus environment can carry a deployment.
        let mut builder = Config::builder()
            .add_source(File::new(&path, FileFormat::Toml).required(explicit || present));

        match env::var(INLINE_CONFIG_VAR) {
            Ok(inline) => {
                builder = builder.add_source(File::from_str(&inline, FileFormat::Toml));
            }
            Err(env::VarError::NotPresent) => {}
            Err(error @ env::VarError::NotUnicode(_)) => {
                return Err(error).with_context(|| {
                    format!("environment variable `{INLINE_CONFIG_VAR}` is not valid Unicode")
                });
            }
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
    pub fn server(&self) -> anyhow::Result<ServerConfig> {
        self.section(SERVER_SECTION, &[])
    }

    /// Extracts the `[telemetry]` section.
    ///
    /// The standard OpenTelemetry variables override the file layers as
    /// aliases.
    pub fn telemetry(&self) -> anyhow::Result<TelemetryConfig> {
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
    /// A missing section yields `T::default()`, and a partial one leaves the
    /// fields it omits at theirs. Unknown fields and invalid values abort
    /// startup, so typos never pass silently.
    ///
    /// Fields are enumerated by serializing `T::default()`, so a config struct
    /// must not use `skip_serializing_if`, and map fields with dynamic keys
    /// cannot be overridden from the environment.
    ///
    /// `aliases` holds `(field path, variable name)` pairs, dots separating
    /// nested segments. They apply before the derived `QUIZ_ARENA_*` names,
    /// which overwrite them.
    pub fn section<T>(&self, name: &str, aliases: &[(&str, &str)]) -> anyhow::Result<T>
    where
        T: DeserializeOwned + Serialize + Default,
    {
        let template = serde_json::to_value(T::default())
            .with_context(|| format!("config section `{name}` has an unserializable default"))?;

        // The file layer overlays the defaults rather than replacing them, so a
        // section that sets one field leaves the rest at the values the
        // template carries. Unknown keys survive the overlay and are what
        // `deny_unknown_fields` later rejects.
        let mut tree = template.clone();
        match self.tree.get::<Value>(name) {
            Ok(value) => {
                if template.is_object() && !value.is_object() {
                    bail!("config section `{name}` must be a table");
                }
                merge(&mut tree, value);
            }
            Err(config::ConfigError::NotFound(_)) => {}
            Err(error) => {
                return Err(error).with_context(|| format!("invalid config section `{name}`"));
            }
        }

        // Validate the file layer before any override, so a failure inside
        // `apply_env` is attributable to its variable.
        serde_json::from_value::<T>(tree.clone())
            .with_context(|| format!("invalid config section `{name}`"))?;

        let mut leaves = Vec::new();
        leaf_paths(&template, &mut Vec::new(), &mut leaves);

        for (field, alias) in aliases {
            let (path, template_leaf) = leaves
                .iter()
                .find(|(path, _)| path.join(".") == *field)
                .with_context(|| {
                    format!("alias `{alias}` targets unknown field `{field}` in section `{name}`")
                })?;
            self.apply_env::<T>(&mut tree, alias, name, path, template_leaf)?;
        }

        for (path, template_leaf) in &leaves {
            let derived = env_var_name(name, path);
            self.apply_env::<T>(&mut tree, &derived, name, path, template_leaf)?;
        }

        serde_json::from_value(tree).with_context(|| format!("invalid config section `{name}`"))
    }

    /// Writes the value of environment variable `var` into `tree` at `path`,
    /// leaving `tree` untouched when the variable is unset.
    ///
    /// Claims the name either way, so an ambiguity is caught whether or not the
    /// variable happens to be set in this environment.
    ///
    /// The value is parsed against the template's JSON shape, which does not
    /// know the field's Rust type, so the tree is deserialized into `T` after
    /// the write to catch values the field rejects, such as an integer out of
    /// range.
    fn apply_env<T: DeserializeOwned>(
        &self,
        tree: &mut Value,
        var: &str,
        section: &str,
        path: &[String],
        template_leaf: &Value,
    ) -> anyhow::Result<()> {
        self.claim(var, section, path)?;
        let Some(raw) = self.env.get(var) else {
            return Ok(());
        };
        let raw = raw
            .to_str()
            .with_context(|| format!("environment variable `{var}` is not valid Unicode"))?;
        let value = parse_env_value(raw, template_leaf)
            .with_context(|| format!("invalid value in environment variable `{var}`"))?;
        insert_at_path(tree, path, value);
        serde_json::from_value::<T>(tree.clone())
            .with_context(|| format!("invalid value in environment variable `{var}`"))?;
        Ok(())
    }

    /// Records that `env_name` resolves to a field of `section`. Two distinct
    /// fields mapping to one variable name is ambiguous, so that aborts
    /// startup.
    fn claim(&self, env_name: &str, section: &str, path: &[String]) -> anyhow::Result<()> {
        let target = if path.is_empty() {
            section.to_owned()
        } else {
            format!("{section}.{}", path.join("."))
        };
        let mut claimed = self.claimed.lock().expect("config claim lock poisoned");
        if let Some(existing) = claimed.get(env_name) {
            if *existing != target {
                bail!(
                    "environment variable `{env_name}` is ambiguous, \
                     it maps to both `{existing}` and `{target}`"
                );
            }
        } else {
            claimed.insert(env_name.to_owned(), target);
        }
        Ok(())
    }
}

/// Overlays `overlay` onto `base`, recursing into tables so a section that sets
/// one field leaves every other at the value it already held.
fn merge(base: &mut Value, overlay: Value) {
    match (base, overlay) {
        (Value::Object(base), Value::Object(overlay)) => {
            for (key, value) in overlay {
                match base.get_mut(&key) {
                    Some(slot) => merge(slot, value),
                    None => {
                        base.insert(key, value);
                    }
                }
            }
        }
        (base, overlay) => *base = overlay,
    }
}

/// Enumerates the leaf paths of a serialized default struct, carrying the
/// default value at each leaf as a type template for parsing.
fn leaf_paths(value: &Value, prefix: &mut Vec<String>, out: &mut Vec<(Vec<String>, Value)>) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                prefix.push(key.clone());
                leaf_paths(child, prefix, out);
                prefix.pop();
            }
        }
        leaf => out.push((prefix.clone(), leaf.clone())),
    }
}

/// `("my_module", ["retry", "max_attempts"])` becomes
/// `QUIZ_ARENA_MY_MODULE_RETRY_MAX_ATTEMPTS`.
fn env_var_name(section: &str, path: &[String]) -> String {
    let mut name = format!("{ENV_PREFIX}{}", section.to_uppercase());
    for segment in path {
        name.push('_');
        name.push_str(&segment.to_uppercase());
    }
    name
}

/// Parses a raw environment string into the JSON shape of the template, which
/// is the field's default value. A null template (an `Option` field) falls back
/// to guessing bool, then number, then string.
fn parse_env_value(raw: &str, template: &Value) -> anyhow::Result<Value> {
    let value = match template {
        Value::Bool(_) => Value::Bool(raw.parse().context("expected a boolean")?),
        Value::Number(number) if number.is_f64() => {
            Value::Number(raw.parse().context("expected a number")?)
        }
        Value::Number(_) => {
            let number = raw
                .parse::<i64>()
                .map(serde_json::Number::from)
                .or_else(|_| raw.parse::<u64>().map(serde_json::Number::from))
                .context("expected an integer")?;
            Value::Number(number)
        }
        Value::String(_) => Value::String(raw.to_owned()),
        Value::Array(_) => {
            Value::Array(serde_json::from_str(raw).context("expected a JSON array")?)
        }
        Value::Null => {
            if let Ok(boolean) = raw.parse::<bool>() {
                Value::Bool(boolean)
            } else if let Ok(number) = raw.parse() {
                Value::Number(number)
            } else {
                Value::String(raw.to_owned())
            }
        }
        Value::Object(_) => unreachable!("objects are never leaf templates"),
    };
    Ok(value)
}

/// Writes `value` at `path`, creating intermediate tables as needed.
fn insert_at_path(tree: &mut Value, path: &[String], value: Value) {
    let Some((last, parents)) = path.split_last() else {
        *tree = value;
        return;
    };
    let mut node = tree;
    for key in parents {
        let map = node.as_object_mut().expect("intermediate nodes are tables");
        node = map
            .entry(key.clone())
            .or_insert_with(|| Value::Object(serde_json::Map::new()));
        // A file may have set a scalar where the struct nests a table. The
        // environment override wins, so replace it.
        if !node.is_object() {
            *node = Value::Object(serde_json::Map::new());
        }
    }
    node.as_object_mut()
        .expect("intermediate nodes are tables")
        .insert(last.clone(), value);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Retry {
        max_attempts: i64,
    }

    impl Default for Retry {
        fn default() -> Self {
            Self { max_attempts: 3 }
        }
    }

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct MyModuleConfig {
        database_url: String,
        port: u16,
        retry: Retry,
        note: Option<String>,
    }

    impl Default for MyModuleConfig {
        fn default() -> Self {
            Self {
                database_url: "sqlite::memory:".to_owned(),
                port: 1234,
                retry: Retry::default(),
                note: None,
            }
        }
    }

    #[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct MyConfig {
        module_x: String,
    }

    /// Deliberately collides with [`MyConfig`] in the environment, `my_module`
    /// + `x` and `my` + `module_x` both derive `QUIZ_ARENA_MY_MODULE_X`.
    #[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct XOnly {
        x: String,
    }

    #[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
    #[serde(deny_unknown_fields)]
    struct UnsignedConfig {
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
        assert_eq!(config, MyModuleConfig::default());
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

        let config = AppConfig::new(
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
        let app = app("", None, &[]);
        let _: MyConfig = app.section("my", &[]).unwrap();
        let error = app.section::<XOnly>("my_module", &[]).unwrap_err();
        assert!(format!("{error:#}").contains("ambiguous"));
    }

    #[test]
    fn unit_config_works_for_configless_sections() {
        let () = app("", None, &[]).section("bare", &[]).unwrap();
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
