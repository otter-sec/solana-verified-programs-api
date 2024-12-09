use diesel_async::pooled_connection::deadpool::{self, PoolError};
use diesel_async::pooled_connection::{deadpool::Pool, AsyncDieselConnectionManager};
use diesel_async::AsyncPgConnection;
use r2d2_redis::{r2d2, RedisConnectionManager};
use std::time::Duration;

use crate::errors::ApiError;

const DEFAULT_POOL_SIZE: usize = 5;
const DEFAULT_TIMEOUT_SECONDS: u64 = 30;

#[derive(Clone)]
pub struct DbClient {
    pub db_pool: Pool<AsyncPgConnection>,
    pub redis_pool: r2d2::Pool<RedisConnectionManager>,
}

impl DbClient {
    pub fn new(db_url: &str, redis_url: &str) -> Self {
        Self::with_config(
            db_url,
            redis_url,
            DEFAULT_POOL_SIZE,
            DEFAULT_TIMEOUT_SECONDS,
        )
    }

    pub fn with_config(
        db_url: &str,
        redis_url: &str,
        pool_size: usize,
        timeout_seconds: u64,
    ) -> Self {
        // Configure PostgreSQL connection
        let config = AsyncDieselConnectionManager::<AsyncPgConnection>::new(db_url);
        let postgres_pool = Pool::builder(config)
            .build()
            .expect("Failed to create DB Pool");

        // Configure Redis connection
        let redis_manager = RedisConnectionManager::new(redis_url)
            .expect("Failed to create Redis connection manager");

        let redis_pool = r2d2::Pool::builder()
            .max_size(pool_size as u32)
            .min_idle(Some(1))
            .max_lifetime(Some(Duration::from_secs(timeout_seconds)))
            .build(redis_manager)
            .expect("Failed to create Redis connection pool");

        Self {
            db_pool: postgres_pool,
            redis_pool,
        }
    }

    /// Get a connection from the Postgres pool with timeout
    pub async fn get_db_conn(&self) -> Result<deadpool::Object<AsyncPgConnection>, PoolError> {
        self.db_pool.get().await
    }

    /// Get a connection from the Redis pool with timeout
    pub fn get_redis_conn(
        &self,
    ) -> Result<r2d2::PooledConnection<RedisConnectionManager>, r2d2::Error> {
        self.redis_pool.get()
    }

    /// Check if both pools are healthy
    pub async fn healthcheck(&self) -> Result<(), ApiError> {
        let postgres_conn = self.db_pool.get().await;
        let redis_conn = self.redis_pool.get();

        if postgres_conn.is_err() || redis_conn.is_err() {
            return Err(ApiError::Custom("Database or Redis connection failed".to_string()));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_db_conn_healthcheck() {
        dotenv::dotenv().ok();
        let db_url = std::env::var("TEST_DATABASE_URL").unwrap();
        let redis_url = std::env::var("TEST_REDIS_URL").unwrap();
        let client = DbClient::new(&db_url, &redis_url);

        let result = client.healthcheck().await;

        assert!(result.is_ok());
    }
}
