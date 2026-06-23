use std::{fmt, string::FromUtf8Error};
use thiserror::Error;

use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

#[derive(Error, Debug)]
pub enum ApiError {
    #[error(transparent)]
    Io(#[from] tokio::io::Error),

    #[error("Failed building: {0}")]
    Build(String),

    /// Malformed input -- 400.
    #[error("{0}")]
    BadRequest(String),

    /// Missing -- 404.
    #[error("{0}")]
    NotFound(String),

    #[error(transparent)]
    ClientError(Box<solana_client::client_error::ClientError>),

    #[error(transparent)]
    ParseAccountError(Box<solana_account_decoder::parse_account_data::ParseAccountError>),

    #[error(transparent)]
    ParsePubkeyError(#[from] solana_pubkey::ParsePubkeyError),

    #[error("Failed parsing utf8 string: {0}")]
    Utf8(#[from] FromUtf8Error),

    #[error(transparent)]
    Db(#[from] sqlx::Error),

    /// Generic internal error
    #[error("Internal error: {0}")]
    Custom(String),
}

/// Error messages for the API Responses
pub enum ErrorMessages {
    Unexpected,
    DB,
    NoPDA,
}

// Use the ErrorMessages enum to display error messages for the API Responses
impl fmt::Display for ErrorMessages {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            ErrorMessages::Unexpected => {
                "We encountered an unexpected error during the verification process."
            }
            ErrorMessages::DB => {
                "An unforeseen database error has occurred, preventing the initiation of the build process. Kindly try again after some time."
            }
            ErrorMessages::NoPDA => {
                "The PDA associated with the given signer was not found. Please try again with a valid signer."
            }
        };
        write!(f, "{message}")
    }
}

// Manual From implementations for boxed error types to reduce enum size
impl From<solana_client::client_error::ClientError> for ApiError {
    fn from(err: solana_client::client_error::ClientError) -> Self {
        ApiError::ClientError(Box::new(err))
    }
}

impl From<solana_account_decoder::parse_account_data::ParseAccountError> for ApiError {
    fn from(err: solana_account_decoder::parse_account_data::ParseAccountError) -> Self {
        ApiError::ParseAccountError(Box::new(err))
    }
}

impl ApiError {
    pub fn status(&self) -> StatusCode {
        match self {
            ApiError::BadRequest(_) | ApiError::ParsePubkeyError(_) => StatusCode::BAD_REQUEST,
            ApiError::NotFound(_) => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = json!({ "status": "error", "error": self.to_string() });
        (self.status(), Json(body)).into_response()
    }
}

pub type Result<T> = std::result::Result<T, ApiError>;
