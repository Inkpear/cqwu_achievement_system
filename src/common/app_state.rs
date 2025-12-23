use sqlx::PgPool;

use crate::utils::{jwt::JwtConfig, schema::SchemaValidatorCache};

pub struct AppState {
    pub pool: PgPool,
    pub jwt_config: JwtConfig,
    pub schema_cache: SchemaValidatorCache,
}

impl AppState {
    pub fn new(pool: PgPool, jwt_config: JwtConfig, schema_cache: SchemaValidatorCache) -> Self {
        Self {
            pool,
            jwt_config,
            schema_cache,
        }
    }
}
