use std::sync::LazyLock;

use argon2::{
    Argon2, PasswordHasher,
    password_hash::{SaltString, rand_core::OsRng},
};
use cqwu_achievement_system::{
    configuration::{DatabaseSettings, get_configuration},
    telemetry::{get_subscriber, init_subscriber},
};

use sqlx::{Connection, Executor, PgConnection, PgPool};
use uuid::Uuid;

static TRACING: LazyLock<()> = LazyLock::new(|| {
    let default_filter_level = "info".to_string();
    let subscriber_name = "test".to_string();

    if std::env::var("TEST_LOG").is_ok() {
        let (subscriber, _) =
            get_subscriber(subscriber_name, default_filter_level, std::io::stdout, None);
        init_subscriber(subscriber);
    } else {
        let (subscriber, _) =
            get_subscriber(subscriber_name, default_filter_level, std::io::sink, None);
        init_subscriber(subscriber);
    }
});

pub struct TestApp {
    pub address: String,
    pub port: u16,
    pub db_pool: PgPool,
    pub api_client: reqwest::Client,
}

impl TestApp {
    pub async fn spawn() -> Self {
        LazyLock::force(&TRACING);

        let configuration = {
            let mut c = get_configuration().expect("Failed to read configuration.");
            c.database.database_name = Uuid::new_v4().to_string();
            c.application.port = 0;
            c
        };

        let api_client = reqwest::Client::builder().build().unwrap();

        let db_pool = configure_database(&configuration.database).await;

        let application =
            cqwu_achievement_system::startup::Application::build(configuration.clone())
                .await
                .expect("Failed to build application.");

        let address = format!(
            "http://{}:{}",
            configuration.application.host,
            application.port()
        );

        let port = application.port();

        let _ = tokio::spawn(application.run_until_stopped());

        TestApp {
            address,
            port,
            db_pool,
            api_client,
        }
    }

    pub async fn post_register(&self, body: &serde_json::Value) -> reqwest::Response {
        self.api_client
            .post(&format!("{}/api/users/register", self.address))
            .json(body)
            .send()
            .await
            .unwrap()
    }
}

pub struct TestUser {
    pub user_id: Option<Uuid>,
    pub username: String,
    pub nickname: String,
    pub password: String,
}

impl TestUser {
    pub fn new() -> Self {
        Self {
            user_id: None,
            username: Uuid::new_v4().to_string(),
            nickname: Uuid::new_v4().to_string(),
            password: Uuid::new_v4().to_string(),
        }
    }

    pub async fn store(&mut self, pool: &PgPool) {
        let salt = SaltString::generate(OsRng);
        let password_hash = Argon2::default()
            .hash_password(self.password.as_bytes(), &salt)
            .unwrap()
            .to_string();
        sqlx::query!(
            "INSERT INTO sys_user (username, nickname, password_hash)
            VALUES ($1, $2, $3)",
            self.username,
            self.nickname,
            password_hash
        )
        .execute(pool)
        .await
        .expect("Failed to store test user");

        let user_id = sqlx::query!(
            "SELECT user_id FROM sys_user WHERE username = $1",
            self.username
        )
        .fetch_one(pool)
        .await
        .expect("Failed to fetch user ID");

        self.user_id = Some(user_id.user_id);
    }
}

async fn configure_database(config: &DatabaseSettings) -> PgPool {
    let mut connection = PgConnection::connect_with(&config.without_db())
        .await
        .expect("Failed to connect to Postgres");

    connection
        .execute(format!(r#"CREATE DATABASE "{}";"#, config.database_name).as_str())
        .await
        .expect("Failed to create database.");

    let connection_pool = PgPool::connect_with(config.with_db())
        .await
        .expect("Failed to connect to Postgres");

    sqlx::migrate!("./migrations")
        .run(&connection_pool)
        .await
        .expect("Failed to migrate database");

    connection_pool
}

pub fn check_response_code_and_message(response: &serde_json::Value, code: u64, msg: &str) {
    assert_eq!(response["code"].as_u64().unwrap(), code);
    assert_eq!(response["message"].as_str().unwrap(), msg);
}
