use deadpool_redis::{
    Connection, Pool, PoolError, Runtime,
    redis::{AsyncTypedCommands, ToRedisArgs},
};
use redis::{FromRedisValue, Script};

use crate::configuration::RedisSettings;

pub struct RedisCache {
    client: Pool,
}

impl RedisCache {
    pub fn from_config(config: &RedisSettings) -> Self {
        let redis_cfg = deadpool_redis::Config::from_url(config.uri.clone());
        let client = redis_cfg
            .create_pool(Some(Runtime::Tokio1))
            .expect("无法创建Redis连接池");

        Self { client }
    }

    async fn get_client(&self) -> Result<Connection, PoolError> {
        self.client.get().await
    }

    pub async fn execute_script<K, A, R>(
        &self,
        script_str: &str,
        keys: &[K],
        args: &[A],
    ) -> Result<R, anyhow::Error>
    where
        K: ToRedisArgs,
        A: ToRedisArgs,
        R: FromRedisValue,
    {
        let mut conn = self.get_client().await?;

        let result: R = Script::new(script_str)
            .key(keys)
            .arg(args)
            .invoke_async(&mut conn)
            .await?;

        Ok(result)
    }

    pub async fn set_ex<K, V>(&self, key: &K, value: &V, exp: u64) -> Result<(), anyhow::Error>
    where
        K: ToRedisArgs + Send + Sync,
        V: ToRedisArgs + Send + Sync,
    {
        let mut conn = self.get_client().await?;
        conn.set_ex(key, value, exp).await?;
        Ok(())
    }

    pub async fn get<K>(&self, key: &K) -> Result<Option<String>, anyhow::Error>
    where
        K: ToRedisArgs + Send + Sync,
    {
        let mut conn = self.get_client().await?;
        let value: Option<String> = conn.get(key).await?;
        Ok(value)
    }

    pub async fn del<K>(&self, key: &K) -> Result<(), anyhow::Error>
    where
        K: ToRedisArgs + Send + Sync,
    {
        let mut conn = self.get_client().await?;
        conn.del(key).await?;
        Ok(())
    }

    pub async fn exists<K>(&self, key: &K) -> Result<bool, anyhow::Error>
    where
        K: ToRedisArgs + Send + Sync,
    {
        let mut conn = self.get_client().await?;
        let exists: bool = conn.exists(key).await?;
        Ok(exists)
    }
}
