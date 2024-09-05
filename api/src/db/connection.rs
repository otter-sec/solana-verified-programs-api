use diesel_async::pooled_connection::AsyncDieselConnectionManager;
use diesel_async::{pooled_connection::deadpool::Pool, AsyncPgConnection};
use r2d2_redis::{r2d2, RedisConnectionManager};

#[derive(Clone)]
pub struct DbClient {
    pub db_pool: Pool<AsyncPgConnection>,
    pub redis_pool: r2d2::Pool<RedisConnectionManager>,
}

impl DbClient {
    pub fn new(db_url: &str, redis_url: &str) -> Self {
        let config = AsyncDieselConnectionManager::<diesel_async::AsyncPgConnection>::new(db_url);
        let postgres_pool = Pool::builder(config)
            .build()
            .expect("Failed to create DB Pool");
        let manager = RedisConnectionManager::new(redis_url).expect(
            "Failed to create Redis connection manager. Check that REDIS_URL is set in .env file",
        );
        let redis_pool = r2d2::Pool::builder().build(manager).expect(
            "Failed to create Redis connection pool. Check that REDIS_URL is set in .env file",
        );

        Self {
            db_pool: postgres_pool,
            redis_pool,
        }
    }
}
