use secrecy::{ExposeSecret, SecretString};
use sqlx::postgres::{PgConnectOptions, PgSslMode};

use crate::utils::jwt::JwtConfig;

#[derive(serde::Deserialize, Clone)]
pub struct DatabaseSettings {
    pub username: String,
    pub password: SecretString,
    pub port: u16,
    pub host: String,
    pub database_name: String,
    pub require_ssl: bool,
    pub max_connections: Option<u32>,
    pub min_connections: Option<u32>,
}

impl DatabaseSettings {
    pub fn without_db(&self) -> PgConnectOptions {
        let ssl_model = if self.require_ssl {
            PgSslMode::Require
        } else {
            PgSslMode::Prefer
        };

        PgConnectOptions::new()
            .host(&self.host)
            .port(self.port)
            .username(&self.username)
            .password(self.password.expose_secret())
            .ssl_mode(ssl_model)
    }

    pub fn with_db(&self) -> PgConnectOptions {
        self.without_db().database(&self.database_name)
    }
}

#[derive(serde::Deserialize, Clone)]
pub struct ApplicationSettings {
    pub port: u16,
    pub host: String,
    pub base_url: String,
}

#[derive(serde::Deserialize, Clone)]
pub struct Settings {
    pub database: DatabaseSettings,
    pub application: ApplicationSettings,
    pub log: LogSettings,
    pub jwt: JwtConfig,
    pub storage: StorageSettings,
    pub redis: RedisSettings,
    #[serde(default)]
    pub tasks: TaskSettings,
}

#[derive(serde::Deserialize, Clone)]
pub struct TaskSettings {
    #[serde(default = "default_orphan_cleanup_interval_seconds")]
    pub orphan_cleanup_interval_seconds: u64,
    #[serde(default = "default_orphan_cleanup_min_age_seconds")]
    pub orphan_cleanup_min_age_seconds: u64,
    #[serde(default = "default_command_queue_size")]
    pub command_queue_size: usize,
    #[serde(default = "default_command_worker_concurrency")]
    pub command_worker_concurrency: usize,
    #[serde(default = "default_command_worker_enabled")]
    pub command_worker_enabled: bool,
    #[serde(default = "default_outbox_pull_interval_millis")]
    pub outbox_pull_interval_millis: u64,
    #[serde(default = "default_outbox_pull_batch_size")]
    pub outbox_pull_batch_size: usize,
    #[serde(default = "default_outbox_running_timeout_seconds")]
    pub outbox_running_timeout_seconds: u64,
}

fn default_orphan_cleanup_interval_seconds() -> u64 {
    60 * 60
}

fn default_orphan_cleanup_min_age_seconds() -> u64 {
    60 * 60
}

fn default_command_queue_size() -> usize {
    1024
}

fn default_command_worker_concurrency() -> usize {
    32
}

fn default_command_worker_enabled() -> bool {
    true
}

fn default_outbox_pull_interval_millis() -> u64 {
    500
}

fn default_outbox_pull_batch_size() -> usize {
    100
}

fn default_outbox_running_timeout_seconds() -> u64 {
    5 * 60
}

impl Default for TaskSettings {
    fn default() -> Self {
        Self {
            orphan_cleanup_interval_seconds: default_orphan_cleanup_interval_seconds(),
            orphan_cleanup_min_age_seconds: default_orphan_cleanup_min_age_seconds(),
            command_queue_size: default_command_queue_size(),
            command_worker_concurrency: default_command_worker_concurrency(),
            command_worker_enabled: default_command_worker_enabled(),
            outbox_pull_interval_millis: default_outbox_pull_interval_millis(),
            outbox_pull_batch_size: default_outbox_pull_batch_size(),
            outbox_running_timeout_seconds: default_outbox_running_timeout_seconds(),
        }
    }
}

enum Environment {
    Local,
    Production,
}

impl Environment {
    pub fn as_str(&self) -> &'static str {
        match self {
            Environment::Local => "local",
            Environment::Production => "production",
        }
    }
}

impl TryFrom<String> for Environment {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        match value.to_lowercase().as_str() {
            "local" => Ok(Self::Local),
            "production" => Ok(Self::Production),
            other => Err(format!(
                "{} Failed to parse environment, please use `local` or `production`",
                other
            )),
        }
    }
}

#[derive(serde::Deserialize, Clone)]
pub struct LogSettings {
    pub log_path: String,
    pub log_prefix: String,
    pub log_level: String,
}
#[derive(serde::Deserialize, Clone)]
pub struct RedisSettings {
    pub uri: String,
}

#[derive(serde::Deserialize, Clone)]
pub struct StorageSettings {
    pub endpoint: String,
    pub region: String,
    pub access_key: String,
    pub secret_key: SecretString,
    pub bucket_name: String,
    pub sig_exp_seconds: u64,
    pub view_exp_seconds: u64,
}

pub fn get_configuration() -> Result<Settings, config::ConfigError> {
    let base_dir = std::env::current_dir().expect("Failed to read current directory");
    let config_dir = base_dir.join("configuration");

    let environment: Environment = std::env::var("APP_ENVIRONMENT")
        .unwrap_or_else(|_| "local".to_string())
        .try_into()
        .expect("Failed to parse environment");

    let settings = config::Config::builder()
        .add_source(config::File::with_name(
            config_dir.join("base").to_str().unwrap(),
        ))
        .add_source(config::File::with_name(
            config_dir.join(environment.as_str()).to_str().unwrap(),
        ))
        .add_source(
            config::Environment::with_prefix("APP")
                .prefix_separator("_")
                .separator("__"),
        )
        .build()?;

    settings.try_deserialize()
}
