use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

use super::JobStatus;

/// Response structure for program verification status
/// Contains all the necessary information about a program's verification state
#[derive(Debug, Serialize, Deserialize)]
pub struct VerificationResponse {
    /// Indicates if the program is currently verified
    pub is_verified: bool,
    /// The hash of the program as it exists on the blockchain
    pub on_chain_hash: String,
    /// The hash of the locally built executable
    pub executable_hash: String,
    /// URL of the GitHub repository containing the program's source code
    pub repo_url: String,
    /// Git commit hash of the verified version
    pub commit: String,
    /// Timestamp of when the program was last verified
    pub last_verified_at: Option<NaiveDateTime>,
    /// Public key of the signer who verified the program
    pub is_frozen: bool,
}

/// Extends VerificationResponse with signer information
/// Used when multiple signers can verify the same program
#[derive(Debug, Serialize, Deserialize)]
pub struct VerificationResponseWithSigner {
    /// Public key of the signer who verified the program
    pub signer: String,
    /// The complete verification response data
    #[serde(flatten)]
    pub verification_response: VerificationResponse,
}

/// General API response status
/// Used to indicate success or failure of operations
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Status {
    /// Operation completed successfully
    Success,
    /// Operation encountered an error
    Error,
}

/// Standard error response structure
/// Used when an operation fails
#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    /// Status will always be Error for this type
    pub status: Status,
    /// Detailed error message explaining what went wrong
    pub error: String,
}

/// Response structure for verification status checks
/// Used when checking the current verification state of a program
#[derive(Debug, Serialize, Deserialize)]
pub struct StatusResponse {
    /// Current verification status of the program
    pub is_verified: bool,
    /// Human-readable status message
    pub message: String,
    /// Current on-chain hash of the program
    pub on_chain_hash: String,
    /// Hash of the locally built executable
    pub executable_hash: String,
    /// URL of the source code repository
    pub repo_url: String,
    /// Git commit hash of the current version
    pub commit: String,
    /// Timestamp of when the program was last verified
    pub last_verified_at: Option<NaiveDateTime>,
}

/// Extended StatusResponse struct to return program frozen status
#[derive(Debug, Serialize, Deserialize)]
pub struct ExtendedStatusResponse {
    #[serde(flatten)]
    pub status: StatusResponse,
    pub is_frozen: bool,
}

/// Response structure for verification job status
/// Used when checking the status of a verification job
#[derive(Debug, Serialize, Deserialize)]
pub struct VerifyResponse {
    /// Current status of the verification job
    pub status: JobStatus,
    /// Unique identifier for tracking the verification job
    pub request_id: String,
    /// Human-readable status message for the job
    pub message: String,
}

/// Wrapper for successful responses
/// Allows for different types of success responses
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SuccessResponse {
    /// Response for program verification status
    Status(StatusResponse),
    /// Response for verification job status
    Verify(VerifyResponse),
    /// Response for listing all verified programs
    StatusAll(Vec<VerificationResponseWithSigner>),
}

/// Conversion implementations for ApiResponse
impl From<StatusResponse> for SuccessResponse {
    fn from(value: StatusResponse) -> Self {
        Self::Status(value)
    }
}

/// Main API response enum
/// Encompasses all possible API response types
#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ApiResponse {
    Success(SuccessResponse),
    Error(ErrorResponse),
}

/// Conversion implementations for ApiResponse
impl From<StatusResponse> for ApiResponse {
    fn from(value: StatusResponse) -> Self {
        Self::Success(SuccessResponse::Status(value))
    }
}

/// Conversion implementations for ApiResponse
impl From<VerifyResponse> for ApiResponse {
    fn from(value: VerifyResponse) -> Self {
        Self::Success(SuccessResponse::Verify(value))
    }
}

/// Conversion implementations for ApiResponse
impl From<ErrorResponse> for ApiResponse {
    fn from(value: ErrorResponse) -> Self {
        Self::Error(value)
    }
}

/// Response structure for job verification status
/// Used to report the status of a verification job
#[derive(Debug, Serialize, Deserialize)]
pub struct JobVerificationResponse {
    /// Current status of the verification job
    pub status: String,
    /// Detailed message about the job status
    pub message: String,
    /// Current on-chain hash of the program
    pub on_chain_hash: String,
    /// Hash of the built executable
    pub executable_hash: String,
    /// URL of the source code repository
    pub repo_url: String,
}

/// Response structure for listing verified programs
/// Used when retrieving all verified programs
#[derive(Debug, Serialize, Deserialize)]
pub struct VerifiedProgramListResponse {
    pub meta: PaginationMeta,
    pub verified_programs: Vec<String>,
}

/// Pagination metadata
#[derive(Debug, Serialize, Deserialize)]
pub struct PaginationMeta {
    pub total: i64,
    pub page: i64,
    pub total_pages: i64,
    pub items_per_page: i64,
    pub has_next_page: bool,
    pub has_prev_page: bool,
}

/// Response structure for individual program status
#[derive(Debug, Serialize, Deserialize)]
pub struct VerifiedProgramStatusResponse {
    /// Program identifier
    pub program_id: String,
    /// Current verification status
    pub is_verified: bool,
    /// Status message
    pub message: String,
    /// Hash of the program on chain
    pub on_chain_hash: String,
    /// Hash of the executable
    pub executable_hash: String,
    /// Last verification timestamp
    pub last_verified_at: Option<NaiveDateTime>,
    /// Repository URL
    pub repo_url: String,
    /// Git commit hash
    pub commit: String,
}

/// Response structure for list of program statuses
#[derive(Debug, Serialize, Deserialize)]
pub struct VerifiedProgramsStatusListResponse {
    /// Operation status
    pub status: Status,
    /// List of program statuses
    pub data: Option<Vec<VerifiedProgramStatusResponse>>,
    /// Error message if any
    pub error: Option<String>,
}
