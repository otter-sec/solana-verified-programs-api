use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

pub use crate::db::JobStatus;

/// Payload posted to webhook when verification completes. `status` is
/// always either `Completed` or `Failed` (in-progress doesn't fire a
/// webhook); the field is typed so call sites can't drift the string.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationWebhookPayload {
    pub request_id: String,
    pub status: JobStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_verified: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub program_id: Option<crate::validation::Address>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub on_chain_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub executable_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verified_at: Option<NaiveDateTime>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Response structure for program verification status
/// Contains all the necessary information about a program's verification state
#[derive(Debug, Default, Serialize, Deserialize)]
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
    /// Indicates if the program is frozen (not upgradeable)
    pub is_frozen: bool,
    /// Indicates if the program is closed (program data account doesn't exist)
    pub is_closed: bool,
}

impl VerificationResponse {
    /// Shapes a `(state, build)` pair into the `/status` response.
    /// Centralises the "is_verified" rule: non-empty on-chain hash +
    /// matching build hash + not closed.
    pub fn from_state_and_build(
        state: Option<&crate::db::ProgramStateRow>,
        build: Option<&crate::db::BuildRow>,
    ) -> Self {
        let on_chain_hash = state
            .and_then(|s| s.on_chain_hash.clone())
            .unwrap_or_default();
        let is_frozen = state.is_some_and(|s| s.is_frozen);
        let is_closed = state.is_some_and(|s| s.is_closed);
        let Some(b) = build else {
            return Self {
                is_verified: false,
                on_chain_hash,
                is_frozen,
                is_closed,
                ..Self::default()
            };
        };
        let is_verified = !on_chain_hash.is_empty()
            && b.executable_hash.as_deref() == Some(on_chain_hash.as_str())
            && !is_closed;
        Self {
            is_verified,
            on_chain_hash,
            executable_hash: b.executable_hash.clone().unwrap_or_default(),
            repo_url: crate::services::misc::build_repository_url(
                &b.repository,
                b.commit_hash.as_deref(),
            ),
            commit: b.commit_hash.clone().unwrap_or_default(),
            last_verified_at: b.completed_at.map(|t| t.naive_utc()),
            is_frozen,
            is_closed,
        }
    }
}

/// Extends VerificationResponse with signer information
/// Used when multiple signers can verify the same program
#[derive(Debug, Serialize, Deserialize)]
pub struct VerificationResponseWithSigner {
    /// Public key of the signer who verified the program
    pub signer: Option<crate::validation::Address>,
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
    pub is_closed: bool,
}

/// Response structure for verification job status
/// Used when checking the status of a verification job
///
/// `request_id` is the build UUID the caller polls `/job/{job_id}` with.
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

impl From<Vec<VerificationResponseWithSigner>> for ApiResponse {
    fn from(value: Vec<VerificationResponseWithSigner>) -> Self {
        Self::Success(SuccessResponse::StatusAll(value))
    }
}

/// `JobVerificationResponse.status`. A superset of [`JobStatus`] with
/// an `Unknown` for "job not found / bad input / DB error".
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum JobReplyStatus {
    InProgress,
    Completed,
    Failed,
    Unknown,
}

impl From<JobStatus> for JobReplyStatus {
    fn from(j: JobStatus) -> Self {
        match j {
            JobStatus::InProgress => Self::InProgress,
            JobStatus::Completed => Self::Completed,
            JobStatus::Failed => Self::Failed,
        }
    }
}

/// Response structure for job verification status
/// Used to report the status of a verification job
///
/// Hash/url fields are empty strings (not null) for in-progress or failed jobs.
#[derive(Debug, Serialize, Deserialize)]
pub struct JobVerificationResponse {
    /// Current status of the verification job
    pub status: JobReplyStatus,
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
    pub error: Option<String>,
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
    pub program_id: crate::validation::Address,
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

/// Builds the response from a `BuildRow` whose hash already equals
/// `program_state.on_chain_hash` (the caller enforces that via SQL JOIN).
impl From<crate::db::BuildRow> for VerifiedProgramStatusResponse {
    fn from(b: crate::db::BuildRow) -> Self {
        let hash = b.executable_hash.unwrap_or_default();
        Self {
            program_id: b.program_id,
            is_verified: true,
            message: "On chain program verified".to_string(),
            on_chain_hash: hash.clone(),
            executable_hash: hash,
            last_verified_at: b.completed_at.map(|t| t.naive_utc()),
            repo_url: crate::services::misc::build_repository_url(
                &b.repository,
                b.commit_hash.as_deref(),
            ),
            commit: b.commit_hash.unwrap_or_default(),
        }
    }
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

/// Path-extraction shape for `/status/{address}` and `/status-all/{address}`.
#[derive(Debug, Deserialize)]
pub struct VerificationStatusParams {
    pub address: crate::validation::Address,
}

/// Query-string shape for `/verified-programs[?search=]`.
#[derive(Debug, Deserialize)]
pub struct VerifiedProgramsQuery {
    #[serde(default)]
    pub search: Option<String>,
}

/// Sweep liveness as observed at request time.
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, Copy)]
pub enum BackgroundJobStatus {
    /// A recent enough sweep cycle landed; the cache is fresh.
    Active,
    /// A sweep ran but not recently enough.
    Inactive,
    /// No sweep has completed yet (also: empty DB).
    #[serde(rename = "unknown")]
    Unknown,
}

/// Health view for `/health/background-jobs`.
#[derive(Debug, Serialize, Deserialize)]
pub struct BackgroundJobHealth {
    pub status: BackgroundJobStatus,
    pub last_program_check: Option<NaiveDateTime>,
    pub message: String,
}
