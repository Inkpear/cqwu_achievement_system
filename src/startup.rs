use std::net::TcpListener;

use actix_web::{App, HttpServer, dev::Server, middleware::from_fn, web};
use sqlx::{PgPool, postgres::PgPoolOptions};
use tracing_actix_web::TracingLogger;

#[cfg(feature = "swagger")]
use utoipa::OpenApi;
#[cfg(feature = "swagger")]
use utoipa_swagger_ui::SwaggerUi;

use crate::{
    common::app_state::AppState,
    configuration::{DatabaseSettings, Settings},
    middleware::auth::mw_authentication,
    modules::{admin, archive, auth, health_check::health_check_handler, user},
    utils::{redis_cache::RedisCache, s3_storage::S3Storage, schema::SchemaContextCache},
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
    server: Server,
}

pub fn get_connection_pool(configuration: &DatabaseSettings) -> PgPool {
    PgPoolOptions::new()
        .acquire_timeout(std::time::Duration::from_secs(2))
        .connect_lazy_with(configuration.with_db())
}

impl Application {
    pub async fn build(configuration: Settings) -> Result<Self, anyhow::Error> {
        let connection_pool = get_connection_pool(&configuration.database);
        let address = format!(
            "{}:{}",
            configuration.application.host, configuration.application.port
        );

        let jwt_config = configuration.jwt;
        let schema_cache = SchemaContextCache::new();
        let s3_storage = S3Storage::from_config(&configuration.storage).await;
        let redis_cache = RedisCache::from_config(&configuration.redis);

        let app_state = AppState::new(
            connection_pool,
            jwt_config,
            schema_cache,
            s3_storage,
            redis_cache,
        );

        let listener = std::net::TcpListener::bind(address)?;
        let port = listener.local_addr().unwrap().port();
        let server = run(listener, app_state).await?;

        Ok(Self { port, server })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub async fn run_until_stopped(self) -> Result<(), std::io::Error> {
        self.server.await
    }
}
