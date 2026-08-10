//! Application configuration.
//!
//! Configuration merges five layers, lowest precedence first:
//!
//! 1. Struct defaults (`Default` impls together with `#[serde(default)]`)
//! 2. A TOML file, `./config.toml` or the path in `QUIZ_ARENA_CONFIG_PATH`
//! 3. Inline TOML in the `QUIZ_ARENA_CONFIG` environment variable
//! 4. Standard alias environment variables a section declares per field,
//!    e.g. `OTEL_EXPORTER_OTLP_ENDPOINT` for `[telemetry]`
//! 5. Individual `QUIZ_ARENA_<SECTION>_<FIELD>` environment variables
//!
//! Each module declares its config shape as `Module::Config`, and the registry
//! extracts the section named `Module::NAME` while registering it. Extraction
//! also resolves layer 4 by deriving one environment variable name per field of
//! the section struct and probing the environment for it. Deriving names from
//! known fields instead of parsing variable names keeps underscores in section
//! and field names unambiguous.

use std::{collections::HashMap, env, net::SocketAddr, sync::Mutex};

use anyhow::{Context as _, bail};
use config::{Config, File, FileFormat, Source as _};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;
use tracing::warn;

const ENV_PREFIX: &str = "QUIZ_ARENA_";
const CONFIG_PATH_VAR: &str = "QUIZ_ARENA_CONFIG_PATH";
const INLINE_CONFIG_VAR: &str = "QUIZ_ARENA_CONFIG";
const DEFAULT_CONFIG_PATH: &str = "config.toml";
const SERVER_SECTION: &str = "server";
const TELEMETRY_SECTION: &str = "telemetry";

/// Project-level settings from the `[server]` section.
#[derive(Debug, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct ServerConfig {
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
#[serde(default, deny_unknown_fields)]
pub(crate) struct TelemetryConfig {
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
    /// The `service.name` resource attribute stamped on every exported signal.
    ///
    /// `OTEL_SERVICE_NAME` overrides the file value.
    pub service_name: String,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            otlp_endpoint: "http://localhost:4317".to_owned(),
            otlp_protocol: OtlpProtocol::Grpc,
            service_name: env!("CARGO_PKG_NAME").to_owned(),
        }
    }
}

/// OTLP transport, named after the values of `OTEL_EXPORTER_OTLP_PROTOCOL`.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub(crate) enum OtlpProtocol {
    #[serde(rename = "grpc")]
    Grpc,
    #[serde(rename = "http/protobuf")]
    HttpProtobuf,
}

/// The merged configuration tree plus the environment snapshot used to resolve
/// per-field overrides.
pub(crate) struct AppConfig {
    tree: Config,
    env: HashMap<String, String>,
    /// Environment variable names claimed by extracted sections, mapped to the
    /// config path each resolves to. Used to detect ambiguous names.
    claimed: Mutex<HashMap<String, String>>,
}

/// Loads the file and inline layers and snapshots the environment.
pub(crate) fn load() -> anyhow::Result<AppConfig> {
    let (path, explicit) = match env::var(CONFIG_PATH_VAR) {
        Ok(path) => (path, true),
        Err(_) => (DEFAULT_CONFIG_PATH.to_owned(), false),
    };

    // An explicitly configured file must exist. The default path is optional
    // because defaults plus environment can carry a deployment.
    let mut builder = Config::builder().add_source(File::with_name(&path).required(explicit));

    if let Ok(inline) = env::var(INLINE_CONFIG_VAR) {
        builder = builder.add_source(File::from_str(&inline, FileFormat::Toml));
    }

    let tree = builder
        .build()
        .with_context(|| format!("failed to load configuration from `{path}`"))?;

    // The whole environment, not just `QUIZ_ARENA_*`, because sections may
    // declare standard alias variables like `OTEL_EXPORTER_OTLP_ENDPOINT`.
    let env = env::vars()
        .filter(|(name, _)| name.as_str() != CONFIG_PATH_VAR && name.as_str() != INLINE_CONFIG_VAR)
        .collect();

    Ok(AppConfig::new(tree, env))
}

impl AppConfig {
    fn new(tree: Config, env: HashMap<String, String>) -> Self {
        Self {
            tree,
            env,
            claimed: Mutex::new(HashMap::new()),
        }
    }

    /// Extracts the `[server]` section.
    pub(crate) fn server(&self) -> anyhow::Result<ServerConfig> {
        self.section(SERVER_SECTION, &[])
    }

    /// Extracts the `[telemetry]` section.
    ///
    /// The standard OpenTelemetry variables override the file layers as
    /// aliases.
    pub(crate) fn telemetry(&self) -> anyhow::Result<TelemetryConfig> {
        self.section(
            TELEMETRY_SECTION,
            &[
                ("otlp_endpoint", "OTEL_EXPORTER_OTLP_ENDPOINT"),
                ("otlp_protocol", "OTEL_EXPORTER_OTLP_PROTOCOL"),
                ("service_name", "OTEL_SERVICE_NAME"),
            ],
        )
    }

    /// Warns about top-level sections that no enabled consumer claims.
    ///
    /// Called from `main` once the tracing subscriber is installed, so the
    /// warnings are actually visible.
    pub(crate) fn warn_unknown_sections(&self) {
        let known: Vec<&str> = [SERVER_SECTION, TELEMETRY_SECTION]
            .into_iter()
            .chain(crate::modules::names())
            .collect();
        let Ok(map) = self.tree.collect() else {
            return;
        };
        for key in map.keys() {
            if !known.contains(&key.as_str()) {
                warn!(
                    "unknown config section `{key}`, known sections are [{}]",
                    known.join(", ")
                );
            }
        }
    }

    /// Extracts the named section into `T`, resolving environment overrides.
    ///
    /// A missing section yields the struct defaults. Unknown fields and invalid
    /// values are errors so that typos abort startup.
    ///
    /// The fields to probe in the environment are enumerated by serializing
    /// `T::default()`. A field hidden from serialization would be invisible
    /// here, so config structs must not use `skip_serializing_if`. Map fields
    /// with dynamic keys cannot be overridden from the environment.
    ///
    /// `aliases` names standard alias environment variables for some fields,
    /// as `(field path, variable name)` pairs with dots separating nested
    /// path segments. Aliases sit between the file layers and the
    /// `QUIZ_ARENA_*` overrides: they are applied first and the derived-name
    /// pass overwrites them.
    pub(crate) fn section<T>(&self, name: &str, aliases: &[(&str, &str)]) -> anyhow::Result<T>
    where
        T: DeserializeOwned + Serialize + Default,
    {
        let template = serde_json::to_value(T::default())
            .with_context(|| format!("config section `{name}` has an unserializable default"))?;

        let mut tree = match self.tree.get::<Value>(name) {
            Ok(value) => value,
            Err(config::ConfigError::NotFound(_)) => template.clone(),
            Err(error) => {
                return Err(error).with_context(|| format!("invalid config section `{name}`"));
            }
        };

        if template.is_object() && !tree.is_object() {
            bail!("config section `{name}` must be a table");
        }

        let mut leaves = Vec::new();
        leaf_paths(&template, &mut Vec::new(), &mut leaves);

        for (field, alias) in aliases {
            let (path, template_leaf) = leaves
                .iter()
                .find(|(path, _)| path.join(".") == *field)
                .with_context(|| {
                    format!("alias `{alias}` targets unknown field `{field}` in section `{name}`")
                })?;
            self.claim(alias, name, path)?;
            if let Some(raw) = self.env.get(*alias) {
                let value = parse_env_value(raw, template_leaf)
                    .with_context(|| format!("invalid value in environment variable `{alias}`"))?;
                insert_at_path(&mut tree, path, value);
            }
        }

        for (path, template_leaf) in &leaves {
            let env_name = env_var_name(name, path);
            self.claim(&env_name, name, path)?;
            if let Some(raw) = self.env.get(&env_name) {
                let value = parse_env_value(raw, template_leaf).with_context(|| {
                    format!("invalid value in environment variable `{env_name}`")
                })?;
                insert_at_path(&mut tree, path, value);
            }
        }

        serde_json::from_value(tree).with_context(|| format!("invalid config section `{name}`"))
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
            Value::Number(raw.parse::<i64>().context("expected an integer")?.into())
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
        // A file may have set a scalar where the struct nests a table.
        // The environment override wins, so replace it.
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
    #[serde(default, deny_unknown_fields)]
    struct Retry {
        max_attempts: i64,
    }

    impl Default for Retry {
        fn default() -> Self {
            Self { max_attempts: 3 }
        }
    }

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    #[serde(default, deny_unknown_fields)]
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
    #[serde(default, deny_unknown_fields)]
    struct MyConfig {
        module_x: String,
    }

    /// Deliberately collides with [`MyConfig`] in the environment,
    /// `my_module` + `x` and `my` + `module_x` both derive
    /// `QUIZ_ARENA_MY_MODULE_X`.
    #[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
    #[serde(default, deny_unknown_fields)]
    struct XOnly {
        x: String,
    }

    fn app(file: &str, inline: Option<&str>, env: &[(&str, &str)]) -> AppConfig {
        let mut builder = Config::builder().add_source(File::from_str(file, FileFormat::Toml));
        if let Some(inline) = inline {
            builder = builder.add_source(File::from_str(inline, FileFormat::Toml));
        }
        AppConfig::new(
            builder.build().unwrap(),
            env.iter()
                .map(|(name, value)| (name.to_string(), value.to_string()))
                .collect(),
        )
    }

    #[test]
    fn missing_section_yields_defaults() {
        let config: MyModuleConfig = app("", None, &[]).section("my_module", &[]).unwrap();
        assert_eq!(config, MyModuleConfig::default());
    }

    #[test]
    fn file_overrides_defaults() {
        let config: MyModuleConfig = app("[my_module]\nport = 9000", None, &[])
            .section("my_module", &[])
            .unwrap();
        assert_eq!(config.port, 9000);
        assert_eq!(config.database_url, "sqlite::memory:");
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
    fn unit_config_works_for_configless_modules() {
        let () = app("", None, &[]).section("bare", &[]).unwrap();
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
