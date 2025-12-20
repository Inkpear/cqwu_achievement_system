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
            .password(&self.password.expose_secret())
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
