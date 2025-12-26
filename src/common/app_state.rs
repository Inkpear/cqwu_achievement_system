use sqlx::PgPool;

use crate::utils::{
    jwt::JwtConfig, redis_cache::RedisCache, s3_storage::S3Storage, schema::SchemaContextCache,
};

pub struct AppState {
    pub pool: PgPool,
    pub jwt_config: JwtConfig,
    pub schema_cache: SchemaContextCache,
    pub s3_storage: S3Storage,
    pub redis_cache: RedisCache,
}

impl AppState {
    pub fn new(
        pool: PgPool,
        jwt_config: JwtConfig,
        schema_cache: SchemaContextCache,
        s3_storage: S3Storage,
        redis_cache: RedisCache,
    ) -> Self {
        Self {
            pool,
            jwt_config,
            schema_cache,
            s3_storage,
            redis_cache,
        }
    }
}
