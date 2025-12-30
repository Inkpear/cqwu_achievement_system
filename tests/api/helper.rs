use std::sync::LazyLock;

use argon2::{
    Argon2, PasswordHasher,
    password_hash::{SaltString, rand_core::OsRng},
};
use cqwu_achievement_system::{
    configuration::{DatabaseSettings, get_configuration},
    domain::UserRole,
    telemetry::{get_subscriber, init_subscriber},
    utils::jwt::JwtConfig,
};
use rand::{Rng, distr::Alphanumeric};
use reqwest::header::HeaderMap;

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
    pub jwt_config: JwtConfig,
    pub database_config: DatabaseSettings,
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

        let api_client = reqwest::Client::new();

        let db_pool = configure_database(&configuration.database).await;

        let application =
            cqwu_achievement_system::startup::Application::build(configuration.clone())
                .await
                .expect("Failed to build application.");

        let jwt_config = configuration.jwt;

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
            jwt_config,
            database_config: configuration.database,
        }
    }

    pub async fn post_create_user(&self, body: &serde_json::Value) -> reqwest::Response {
        self.api_client
            .post(&format!("{}/api/admin/user/create", self.address))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request")
    }

    pub async fn post_login<Body: serde::Serialize>(&self, form: &Body) -> reqwest::Response {
        self.api_client
            .post(&format!("{}/api/auth/login", self.address))
            .form(form)
            .send()
            .await
            .expect("Failed to execute request")
    }

    pub async fn login(&mut self, user: &TestUser) {
        let body = serde_json::json!({
            "username": user.username,
            "password": user.password,
        });

        let jwt = self
            .post_login(&body)
            .await
            .json::<serde_json::Value>()
            .await
            .expect("Failed to parse login response")["data"]["token"]
            .as_str()
            .unwrap()
            .to_string();

        let mut headers = HeaderMap::new();
        headers.insert("Authorization", format!("Bearer {}", jwt).parse().unwrap());

        self.api_client = reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .expect("Failed to build client with headers");
    }

    pub async fn patch_change_password<Body: serde::Serialize>(
        &self,
        body: &Body,
    ) -> reqwest::Response {
        self.api_client
            .patch(&format!("{}/api/user/password", self.address))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request")
    }

    pub async fn patch_modify_user_status(&self, body: &serde_json::Value) -> reqwest::Response {
        self.api_client
            .patch(&format!("{}/api/admin/user/modify_status", self.address))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request")
    }

    pub async fn post_grant_user_api_rule(&self, body: &serde_json::Value) -> reqwest::Response {
        self.api_client
            .post(&format!("{}/api/admin/api_rule/grant", self.address))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request")
    }

    pub async fn delete_revoke_user_api_rule(&self, rule_id: &str) -> reqwest::Response {
        self.api_client
            .delete(&format!(
                "{}/api/admin/api_rule/revoke/{}",
                self.address, rule_id
            ))
            .send()
            .await
            .expect("Failed to execute request")
    }

    pub async fn get_query_user_api_rules(
        &self,
        user_id: Option<&str>,
        page: i64,
        page_size: i64,
    ) -> reqwest::Response {
        let mut request = self
            .api_client
            .get(&format!("{}/api/admin/api_rule/query", self.address));

        if let Some(uid) = user_id {
            request = request.query(&[("user_id", uid)]);
        }
        request = request.query(&[("page", &page.to_string())]);
        request = request.query(&[("page_size", &page_size.to_string())]);

        request.send().await.expect("Failed to execute request")
    }

    pub async fn get_query_user(&self, user_id: &serde_json::Value) -> reqwest::Response {
        self.api_client
            .get(&format!("{}/api/admin/user/query", self.address))
            .query(user_id)
            .send()
            .await
            .expect("Failed to execute request")
    }

    pub async fn patch_change_user_password(&self, body: &serde_json::Value) -> reqwest::Response {
        self.api_client
            .patch(&format!("{}/api/admin/user/password", self.address))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request")
    }

    pub async fn post_create_template(&self, body: &serde_json::Value) -> reqwest::Response {
        self.api_client
            .post(&format!("{}/api/admin/template/create", self.address))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request")
    }

    pub async fn get_query_templates(
        &self,
        template_id: Option<&str>,
        name: Option<&str>,
        category: Option<&str>,
        page: i64,
        page_size: i64,
    ) -> reqwest::Response {
        let mut request = self
            .api_client
            .get(&format!("{}/api/admin/template/query", self.address));

        if let Some(id) = template_id {
            request = request.query(&[("template_id", id)]);
        }
        if let Some(n) = name {
            request = request.query(&[("name", n)]);
        }
        if let Some(c) = category {
            request = request.query(&[("category", c)]);
        }
        request = request.query(&[("page", &page.to_string())]);
        request = request.query(&[("page_size", &page_size.to_string())]);

        request.send().await.expect("Failed to execute request")
    }

    pub async fn patch_update_template(&self, body: &serde_json::Value) -> reqwest::Response {
        self.api_client
            .patch(&format!("{}/api/admin/template/update", self.address))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request")
    }

    pub async fn delete_template(&self, template_id: &str) -> reqwest::Response {
        self.api_client
            .delete(&format!(
                "{}/api/admin/template/delete/{}",
                self.address, template_id
            ))
            .send()
            .await
            .expect("Failed to execute request")
    }

    pub async fn patch_modify_template_status(
        &self,
        body: &serde_json::Value,
    ) -> reqwest::Response {
        self.api_client
            .patch(&format!(
                "{}/api/admin/template/modify_status",
                self.address
            ))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request")
    }

    pub async fn post_create_archive_record(
        &self,
        template_id: &str,
        body: &serde_json::Value,
    ) -> reqwest::Response {
        self.api_client
            .post(&format!(
                "{}/api/archive/{}/create",
                self.address, template_id
            ))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request")
    }

    pub async fn post_query_archive_records(
        &self,
        template_id: &str,
        body: &serde_json::Value,
    ) -> reqwest::Response {
        self.api_client
            .post(&format!(
                "{}/api/archive/{}/query",
                self.address, template_id
            ))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request")
    }

    pub async fn get_init_upload_session(&self, template_id: &str) -> reqwest::Response {
        self.api_client
            .get(&format!(
                "{}/api/archive/{}/init_upload",
                self.address, template_id
            ))
            .send()
            .await
            .expect("Failed to execute request")
    }

    pub async fn post_presigned_upload_url(
        &self,
        template_id: &str,
        body: &serde_json::Value,
    ) -> reqwest::Response {
        self.api_client
            .post(&format!(
                "{}/api/archive/{}/presigned",
                self.address, template_id
            ))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request")
    }

    pub async fn put_to_upload_file(
        &self,
        upload_url: &str,
        file_content: &Vec<u8>,
        content_type: &str,
        filename: &str,
    ) -> reqwest::Response {
        let filename = urlencoding::encode(filename);
        reqwest::Client::new()
            .put(upload_url)
            .header("Content-Type", content_type)
            .header("x-amz-meta-original-filename", filename.as_ref())
            .body(file_content.clone())
            .send()
            .await
            .expect("Failed to execute request")
    }

    pub async fn delete_archive_record(&self, record_id: &str) -> reqwest::Response {
        self.api_client
            .delete(&format!(
                "{}/api/archive/{}/delete",
                self.address, record_id
            ))
            .send()
            .await
            .expect("Failed to execute request")
    }

    pub async fn get_all_template_categories(&self) -> reqwest::Response {
        self.api_client
            .get(&format!(
                "{}/api/admin/template/all_categories",
                self.address
            ))
            .send()
            .await
            .expect("Failed to execute request")
    }

    pub async fn post_to_presigned_avatar_url(
        &self,
        body: &serde_json::Value,
    ) -> reqwest::Response {
        self.api_client
            .post(&format!("{}/api/user/avatar/presigned", self.address))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request")
    }

    pub async fn patch_to_update_avatar(&self, file_id: &str) -> reqwest::Response {
        self.api_client
            .patch(&format!("{}/api/user/avatar/{}", self.address, file_id))
            .send()
            .await
            .expect("Failed to execute request")
    }

    pub async fn patch_to_update_user_info(
        &self,
        body: &serde_json::Value,
    ) -> reqwest::Response {
        self.api_client
            .patch(&format!("{}/api/user/update", self.address))
            .json(body)
            .send()
            .await
            .expect("Failed to execute request")
    }

    pub async fn get_user_info(&self) -> reqwest::Response {
        self.api_client
            .get(&format!("{}/api/user/me", self.address))
            .send()
            .await
            .expect("Failed to execute request")
    }

    pub async fn get_user_effective_routes(&self) -> reqwest::Response {
        self.api_client
            .get(&format!("{}/api/user/routes", self.address))
            .send()
            .await
            .expect("Failed to execute request")
    }
}

pub struct TestUser {
    pub user_id: Option<Uuid>,
    pub username: String,
    pub nickname: String,
    pub password: String,
    pub role: UserRole,
}

impl TestUser {
    pub fn new() -> Self {
        Self {
            user_id: None,
            username: Uuid::new_v4().to_string(),
            nickname: Uuid::new_v4().to_string(),
            password: Uuid::new_v4().to_string(),
            role: UserRole::USER,
        }
    }

    pub async fn default_admin(pool: &PgPool) -> Self {
        let row = sqlx::query!("SELECT user_id FROM sys_user WHERE username = 'admin'")
            .fetch_one(pool)
            .await
            .expect("Failed to fetch admin ID");

        Self {
            user_id: Some(row.user_id),
            username: "admin".to_string(),
            nickname: "系统管理员".to_string(),
            password: "admin123".to_string(),
            role: UserRole::ADMIN,
        }
    }

    pub fn new_admin() -> Self {
        Self {
            user_id: None,
            username: Uuid::new_v4().to_string(),
            nickname: Uuid::new_v4().to_string(),
            password: Uuid::new_v4().to_string(),
            role: UserRole::ADMIN,
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
    assert_eq!(
        response["code"].as_u64().unwrap(),
        code,
        "code not match in\nresponse = {:#?}",
        response
    );
    assert!(
        response["message"].as_str().unwrap().contains(msg),
        "message: {} is not match in\nresponse = {:#?}",
        msg,
        response
    );
}

pub fn generate_a_dummy_file_content(file_size: usize) -> Vec<u8> {
    let mut rng = rand::rng();
    vec![rng.sample(Alphanumeric); file_size]
}
