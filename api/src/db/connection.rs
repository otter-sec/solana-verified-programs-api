use diesel_async::pooled_connection::deadpool::{self, PoolError};
use diesel_async::pooled_connection::{deadpool::Pool, AsyncDieselConnectionManager};
use diesel_async::AsyncPgConnection;
use redis::aio::MultiplexedConnection;
use std::sync::Arc;
use tokio::sync::Mutex;

const DEFAULT_POOL_SIZE: usize = 20;
const DEFAULT_TIMEOUT_SECONDS: u64 = 30;

#[derive(Clone)]
pub struct DbClient {
    pub db_pool: Pool<AsyncPgConnection>,
    pub async_redis_conn: Arc<Mutex<Option<MultiplexedConnection>>>, // New async Redis connection
    redis_url: String, // Store Redis URL for async connection
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
        _timeout_seconds: u64,
    ) -> Self {
        // Configure PostgreSQL connection with pool settings
        let config = AsyncDieselConnectionManager::<AsyncPgConnection>::new(db_url);
        let postgres_pool = Pool::builder(config)
            .max_size(pool_size)
            .build()
            .expect("Failed to create DB Pool");

        Self {
            db_pool: postgres_pool,
            async_redis_conn: Arc::new(Mutex::new(None)),
            redis_url: redis_url.to_string(),
        }
    }

    /// Get a connection from the Postgres pool with timeout
    pub async fn get_db_conn(&self) -> Result<deadpool::Object<AsyncPgConnection>, PoolError> {
        self.db_pool.get().await
    }

    /// Get async Redis connection (creates one if it doesn't exist)
    pub async fn get_async_redis_conn(&self) -> Result<MultiplexedConnection, redis::RedisError> {
        let mut conn_guard = self.async_redis_conn.lock().await;

        if conn_guard.is_none() {
            let client = redis::Client::open(self.redis_url.as_str())?;
            let multiplexed_conn = client.get_multiplexed_async_connection().await?;
            *conn_guard = Some(multiplexed_conn);
        }

        // Clone the connection (it's designed to be cloned)
        Ok(conn_guard.as_ref().unwrap().clone())
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

        let postgres_conn = client.get_db_conn().await;
        let redis_conn = client.get_async_redis_conn().await;

        assert!(postgres_conn.is_ok());
        assert!(redis_conn.is_ok());
    }
}
