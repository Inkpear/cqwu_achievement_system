use std::sync::Arc;

use sqlx::PgPool;

use crate::utils::{redis_cache::RedisCache, s3_storage::S3Storage};

#[derive(Clone)]
pub struct TaskExecutionContext {
    pool: Arc<PgPool>,
    s3_storage: Arc<S3Storage>,
    redis_cache: Arc<RedisCache>,
}

impl TaskExecutionContext {
    pub fn new(
        pool: Arc<PgPool>,
        s3_storage: Arc<S3Storage>,
        redis_cache: Arc<RedisCache>,
    ) -> Self {
        Self {
            pool,
            s3_storage,
            redis_cache,
        }
    }

    pub fn pool(&self) -> &PgPool {
        self.pool.as_ref()
    }

    pub fn s3_storage(&self) -> &S3Storage {
        self.s3_storage.as_ref()
    }

    pub fn redis_cache(&self) -> &RedisCache {
        self.redis_cache.as_ref()
    }
}
