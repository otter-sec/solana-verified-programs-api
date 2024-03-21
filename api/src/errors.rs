use std::{fmt, string::FromUtf8Error};
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
    RedisError(#[from] redis::RedisError),

    #[error(transparent)]
    RedisPool(#[from] r2d2_redis::redis::RedisError),

    #[error(transparent)]
    DbPool(#[from] diesel_async::pooled_connection::deadpool::PoolError),
}

pub enum ErrorMessages {
    Unexpected,
    DB,
}

impl fmt::Display for ErrorMessages {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            ErrorMessages::Unexpected => "We encountered an unexpected error during the verification process.",
            ErrorMessages::DB => "An unforeseen database error has occurred, preventing the initiation of the build process. Kindly try again after some time.",
        };
        write!(f, "{}", message)
    }
}
