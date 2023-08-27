use std::string::FromUtf8Error;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ApiError {
    #[error(transparent)]
    Io(#[from] tokio::io::Error),

    #[error("Failed building: {0}")]
    Build(String),

    #[error("Unexpected Error: {0}")]
    Custom(String),

    #[error("Failed parsing utf8 string: {0}")]
    Utf8(#[from] FromUtf8Error),

    #[error(transparent)]
    Diesel(#[from] diesel::result::Error),

    #[error(transparent)]
    Redis(#[from] r2d2_redis::r2d2::Error),

    #[error(transparent)]
    RedisPool(#[from] r2d2_redis::redis::RedisError),

    #[error(transparent)]
    DbPool(#[from] diesel_async::pooled_connection::deadpool::PoolError),
}
