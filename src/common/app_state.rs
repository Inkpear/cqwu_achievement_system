use sqlx::PgPool;

use crate::utils::jwt::JwtConfig;

pub struct AppState {
    pub pool: PgPool,
    pub jwt_config: JwtConfig,
}

impl AppState {
    pub fn new(pool: PgPool, jwt_config: JwtConfig) -> Self {
        Self { pool, jwt_config }
    }
}
