#[derive(Debug)]
pub enum ApiError {
    Custom(String),
    BuildError,
    ParseError(String),
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            ApiError::Custom(msg) => write!(f, "{}", msg),
            ApiError::BuildError => write!(f, "Failed to build the program"),
            ApiError::ParseError(msg) => write!(f, "Failed : {}", msg),
        }
    }
}

impl std::error::Error for ApiError {}
