use std::net::TcpListener;
use std::sync::Arc;

use actix_web::{App, HttpServer, dev::Server, middleware::from_fn, web};
use secrecy::{ExposeSecret, SecretString};
use sqlx::{PgPool, postgres::PgPoolOptions};
use tracing_actix_web::TracingLogger;
use uuid::Uuid;

#[cfg(feature = "swagger")]
use utoipa::OpenApi;
#[cfg(feature = "swagger")]
use utoipa_swagger_ui::SwaggerUi;

use crate::{
    common::app_state::AppState,
    configuration::{DatabaseSettings, Settings, TaskSettings},
    middleware::auth::mw_authentication,
    modules::{admin, archive, auth, health_check::health_check_handler, user},
    tasks::{
        dispatcher::TaskDispatcher,
        handler::DefaultTaskHandler,
        manager::TaskManager,
        models::RetryConfig,
        periodic::{outbox_pull::pull_outbox_tasks, s3_cleanup::cleanup_orphan_persistent_objects},
    },
    utils::{
        jwt::JwtConfig, password::hash_password, redis_cache::RedisCache, s3_storage::S3Storage,
        schema::SchemaContextCache,
    },
};

#[cfg(feature = "swagger")]
static SWAGGER_INFO: std::sync::LazyLock<()> = std::sync::LazyLock::new(|| {
    tracing::info!("Swagger UI will be available at /swagger-ui/");
});

#[cfg(feature = "swagger")]
use crate::documentation::ApiDoc;

pub async fn run(listener: TcpListener, app_state: AppState) -> Result<Server, anyhow::Error> {
    let app_state = web::Data::new(app_state);

    #[cfg(feature = "swagger")]
    let openapi = ApiDoc::openapi();

    let server = HttpServer::new(move || {
        let mut app = App::new()
            .wrap(TracingLogger::default())
            .app_data(app_state.clone());

        #[cfg(feature = "swagger")]
        {
            app = app.service(
                SwaggerUi::new("/swagger-ui/{_:.*}").url("/api-docs/openapi.json", openapi.clone()),
            );
            std::sync::LazyLock::force(&SWAGGER_INFO);
        }

        app = app
            .route("/health_check", web::get().to(health_check_handler))
            .route(
                "/api/auth/login",
                web::post().to(auth::routes::login_user_handler),
            )
            // protected routes
            .service(
                web::scope("/api")
                    .wrap(from_fn(mw_authentication))
                    .configure(admin::config)
                    .configure(user::config)
                    .configure(archive::config),
            );

        app
    })
    .listen(listener)?
    .run();

    Ok(server)
}

pub struct Application {
    port: u16,
    task_manager: TaskManager,
    server: Server,
}

struct InitializedApplication {
    address: String,
    connection_pool: PgPool,
    jwt_config: JwtConfig,
    schema_cache: SchemaContextCache,
    s3_storage: S3Storage,
    redis_cache: RedisCache,
    task_settings: TaskSettings,
}

struct TaskRuntime {
    manager: TaskManager,
    dispatcher: TaskDispatcher,
}

impl InitializedApplication {
    fn into_address_and_state(
        self,
        task_dispatcher: crate::tasks::dispatcher::TaskDispatcher,
    ) -> (String, AppState) {
        let app_state = AppState::new(
            self.connection_pool,
            self.jwt_config,
            self.schema_cache,
            self.s3_storage,
            self.redis_cache,
            task_dispatcher,
        );

        (self.address, app_state)
    }
}

pub fn get_connection_pool(configuration: &DatabaseSettings) -> PgPool {
    PgPoolOptions::new()
        .max_connections(configuration.max_connections.unwrap_or(32))
        .min_connections(configuration.min_connections.unwrap_or(5))
        .acquire_timeout(std::time::Duration::from_secs(2))
        .connect_lazy_with(configuration.with_db())
}

impl Application {
    pub async fn build(configuration: Settings) -> Result<Self, anyhow::Error> {
        let initialized = Self::initialize_application(configuration).await;

        Self::run_migrations(&initialized.connection_pool).await?;
        Self::ensure_bootstrap_admin_if_empty(&initialized.connection_pool).await?;

        let mut task_runtime = Self::setup_task_runtime(
            &initialized.connection_pool,
            &initialized.s3_storage,
            &initialized.redis_cache,
            &initialized.task_settings,
        )
        .await;

        Self::register_periodic_tasks(
            &mut task_runtime.manager,
            &initialized.connection_pool,
            &initialized.s3_storage,
            &initialized.task_settings,
        );

        let (address, app_state) = initialized.into_address_and_state(task_runtime.dispatcher);
        let (port, server) = Self::bind_server(address, app_state).await?;

        Ok(Self {
            port,
            server,
            task_manager: task_runtime.manager,
        })
    }

    async fn initialize_application(configuration: Settings) -> InitializedApplication {
        let task_settings = Self::sanitize_task_settings(configuration.tasks.clone());
        let address = format!(
            "{}:{}",
            configuration.application.host, configuration.application.port
        );

        InitializedApplication {
            address,
            connection_pool: get_connection_pool(&configuration.database),
            jwt_config: configuration.jwt,
            schema_cache: SchemaContextCache::new(),
            s3_storage: S3Storage::from_config(&configuration.storage).await,
            redis_cache: RedisCache::from_config(&configuration.redis),
            task_settings,
        }
    }

    async fn run_migrations(connection_pool: &PgPool) -> Result<(), anyhow::Error> {
        sqlx::migrate!("./migrations")
            .run(connection_pool)
            .await
            .map_err(|e| anyhow::anyhow!("failed to run database migrations: {e}"))
    }

    async fn ensure_bootstrap_admin_if_empty(
        connection_pool: &PgPool,
    ) -> Result<(), anyhow::Error> {
        let user_count: i64 = sqlx::query_scalar("SELECT COUNT(*)::BIGINT FROM sys_user")
            .fetch_one(connection_pool)
            .await
            .map_err(|e| anyhow::anyhow!("failed to query sys_user count: {e}"))?;

        if user_count > 0 {
            return Ok(());
        }

        let username = format!(
            "bootstrap_admin_{}",
            &Uuid::new_v4().simple().to_string()[..8]
        );
        let plain_password = format!("Adm!{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
        let password_hash = hash_password(SecretString::from(plain_password.clone())).await?;

        let inserted_id: Option<Uuid> = sqlx::query_scalar(
            r#"
                INSERT INTO sys_user (username, nickname, password_hash, role, is_active)
                SELECT $1, $2, $3, 'ADMIN', TRUE
                WHERE NOT EXISTS (SELECT 1 FROM sys_user)
                RETURNING user_id
            "#,
        )
        .bind(&username)
        .bind("系统管理员")
        .bind(password_hash.expose_secret())
        .fetch_optional(connection_pool)
        .await
        .map_err(|e| anyhow::anyhow!("failed to bootstrap admin user: {e}"))?;

        if inserted_id.is_some() {
            tracing::warn!(
                "sys_user is empty, bootstrap admin created. username='{}', password='{}'. Please login and rotate credentials immediately.",
                username,
                plain_password
            );
        }

        Ok(())
    }

    fn sanitize_task_settings(mut task_settings: TaskSettings) -> TaskSettings {
        let default_task_settings = TaskSettings::default();

        if task_settings.orphan_cleanup_interval_seconds == 0 {
            tracing::warn!(
                "invalid tasks.orphan_cleanup_interval_seconds=0, fallback to default {}",
                default_task_settings.orphan_cleanup_interval_seconds
            );
            task_settings.orphan_cleanup_interval_seconds =
                default_task_settings.orphan_cleanup_interval_seconds;
        }

        if task_settings.orphan_cleanup_min_age_seconds == 0 {
            tracing::warn!(
                "invalid tasks.orphan_cleanup_min_age_seconds=0, fallback to default {}",
                default_task_settings.orphan_cleanup_min_age_seconds
            );
            task_settings.orphan_cleanup_min_age_seconds =
                default_task_settings.orphan_cleanup_min_age_seconds;
        }

        if task_settings.command_queue_size == 0 {
            tracing::warn!(
                "invalid tasks.command_queue_size=0, fallback to default {}",
                default_task_settings.command_queue_size
            );
            task_settings.command_queue_size = default_task_settings.command_queue_size;
        }

        if task_settings.command_worker_concurrency == 0 {
            tracing::warn!(
                "invalid tasks.command_worker_concurrency=0, fallback to default {}",
                default_task_settings.command_worker_concurrency
            );
            task_settings.command_worker_concurrency =
                default_task_settings.command_worker_concurrency;
        }

        if task_settings.outbox_pull_interval_millis == 0 {
            tracing::warn!(
                "invalid tasks.outbox_pull_interval_millis=0, fallback to default {}",
                default_task_settings.outbox_pull_interval_millis
            );
            task_settings.outbox_pull_interval_millis =
                default_task_settings.outbox_pull_interval_millis;
        }

        if task_settings.outbox_pull_batch_size == 0 {
            tracing::warn!(
                "invalid tasks.outbox_pull_batch_size=0, fallback to default {}",
                default_task_settings.outbox_pull_batch_size
            );
            task_settings.outbox_pull_batch_size = default_task_settings.outbox_pull_batch_size;
        }

        if task_settings.outbox_running_timeout_seconds == 0 {
            tracing::warn!(
                "invalid tasks.outbox_running_timeout_seconds=0, fallback to default {}",
                default_task_settings.outbox_running_timeout_seconds
            );
            task_settings.outbox_running_timeout_seconds =
                default_task_settings.outbox_running_timeout_seconds;
        }

        task_settings
    }

    async fn setup_task_runtime(
        connection_pool: &PgPool,
        s3_storage: &S3Storage,
        redis_cache: &RedisCache,
        task_settings: &TaskSettings,
    ) -> TaskRuntime {
        let (mut task_manager, task_dispatcher) = TaskManager::new_pair(
            task_settings.command_queue_size,
            Arc::new(connection_pool.clone()),
        );

        let task_handler = DefaultTaskHandler::new(
            task_dispatcher.clone(),
            Arc::new(connection_pool.clone()),
            Arc::new(s3_storage.clone()),
            Arc::new(redis_cache.clone()),
            RetryConfig::default(),
        );

        if task_settings.command_worker_enabled {
            task_manager.add_command_worker_with_concurrency(
                task_handler,
                task_settings.command_worker_concurrency,
            );

            let pull_dispatcher = task_dispatcher.clone();
            let pull_batch_size = task_settings.outbox_pull_batch_size;
            let running_timeout =
                std::time::Duration::from_secs(task_settings.outbox_running_timeout_seconds);
            task_manager.add_quiet_interval_task(
                "pull_outbox_tasks",
                move || {
                    let dispatcher = pull_dispatcher.clone();
                    async move { pull_outbox_tasks(&dispatcher, pull_batch_size, running_timeout).await }
                },
                std::time::Duration::from_millis(task_settings.outbox_pull_interval_millis),
            );

            match task_dispatcher
                .recover_outbox_tasks(std::time::Duration::from_secs(
                    task_settings.outbox_running_timeout_seconds,
                ))
                .await
            {
                Ok(result) => {
                    if result.reclaimed_running > 0 {
                        tracing::warn!(
                            "reclaimed {} stale running task(s) from outbox during startup",
                            result.reclaimed_running
                        );
                    }

                    if result.recovered_pending > 0 {
                        tracing::info!(
                            "recovered {} task(s) from outbox",
                            result.recovered_pending
                        );
                    }
                }
                Err(e) => {
                    tracing::warn!("failed to recover task outbox during startup: {:?}", e);
                }
            }
        } else {
            task_manager.close_command_channel();
            tracing::warn!(
                "tasks.command_worker_enabled=false, command worker and producer channel are disabled; startup outbox recovery is skipped"
            );
        }

        TaskRuntime {
            manager: task_manager,
            dispatcher: task_dispatcher,
        }
    }

    fn register_periodic_tasks(
        task_manager: &mut TaskManager,
        connection_pool: &PgPool,
        s3_storage: &S3Storage,
        task_settings: &TaskSettings,
    ) {
        let cleanup_pool = connection_pool.clone();
        let cleanup_s3 = s3_storage.clone();
        let min_object_age =
            std::time::Duration::from_secs(task_settings.orphan_cleanup_min_age_seconds);
        task_manager.add_interval_task(
            "cleanup_orphan_persistent_objects",
            move || {
                let pool = cleanup_pool.clone();
                let s3 = cleanup_s3.clone();
                async move { cleanup_orphan_persistent_objects(&pool, &s3, min_object_age).await }
            },
            std::time::Duration::from_secs(task_settings.orphan_cleanup_interval_seconds),
        );
    }

    async fn bind_server(
        address: String,
        app_state: AppState,
    ) -> Result<(u16, Server), anyhow::Error> {
        let listener = std::net::TcpListener::bind(address)?;
        let port = listener.local_addr().unwrap().port();
        let server = run(listener, app_state).await?;

        Ok((port, server))
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub async fn run_until_stopped(mut self) -> Result<(), std::io::Error> {
        let server_result = self.server.await;
        self.task_manager.shutdown().await;

        server_result
    }
}
