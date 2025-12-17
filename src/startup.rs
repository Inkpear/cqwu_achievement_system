use std::net::TcpListener;

use actix_web::{App, HttpServer, dev::Server, web};
use sqlx::{PgPool, postgres::PgPoolOptions};
use tracing_actix_web::TracingLogger;

use crate::{
    configuration::{DatabaseSettings, Settings},
    modules::health_check::health_check_handler,
};

pub async fn run(listener: TcpListener, pool: PgPool) -> Result<Server, anyhow::Error> {
    let pool = web::Data::new(pool);

    let server = HttpServer::new(move || {
        App::new()
            .wrap(TracingLogger::default())
            .app_data(pool.clone())
            .service(health_check_handler)
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

        let listener = std::net::TcpListener::bind(address)?;
        let port = listener.local_addr().unwrap().port();
        let server = run(listener, connection_pool).await?;

        Ok(Self { port, server })
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    pub async fn run_until_stopped(self) -> Result<(), std::io::Error> {
        self.server.await
    }
}
