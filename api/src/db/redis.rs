
use r2d2_redis::redis::{Commands, FromRedisValue, Value};
use crate::errors::ApiError;
use crate::Result;
use super::DbClient;

impl DbClient {
    pub async fn set_cache(&self, program_address: &str, value: &str) -> Result<()> {
        let cache_res = self.redis_pool.get();
        let mut redis_conn = match cache_res {
            Ok(conn) => conn,
            Err(err) => {
                tracing::error!("Redis connection error: {}", err);
                return Err(ApiError::from(err));
            }
        };
        redis_conn
            .set_ex(program_address, value, 60)
            .map_err(|err| {
                tracing::error!("Redis SET failed: {}", err);
                ApiError::from(err)
            })?;
        tracing::info!("Cache set for program: {}", program_address);
        Ok(())
    }

    pub async fn get_cache(&self, program_address: &str) -> Result<String> {
        let cache_res = self.redis_pool.get().map_err(|err| {
            tracing::error!("Redis connection error: {}", err);
            ApiError::from(err)
        })?;

        let mut redis_conn = cache_res;

        let value: Value = redis_conn.get(program_address).map_err(|err| {
            tracing::error!("Redis connection error: {}", err);
            ApiError::from(err)
        })?;

        match value {
            Value::Nil => Err(ApiError::Custom(format!(
                "Record not found for program: {}",
                program_address
            ))),
            _ => FromRedisValue::from_redis_value(&value).map_err(|err| {
                tracing::error!("Redis Value error: {}", err);
                ApiError::from(err)
            }),
        }
    }

    pub async fn check_cache(&self, hash: &str, program_address: &str) -> Result<bool> {
        let cache_res = self.get_cache(program_address).await;
        match cache_res {
            Ok(res) => {
                if res == hash {
                    tracing::info!(
                        "Cache hit for program: {} And hash matches",
                        program_address
                    );
                    Ok(true)
                } else {
                    tracing::info!(
                        "Cache hit for program: {} And hash doesn't matches",
                        program_address
                    );
                    Ok(false)
                }
            }
            Err(err) => {
                tracing::error!("Redis connection error: {}", err);
                Ok(false)
            }
        }
    }
}
