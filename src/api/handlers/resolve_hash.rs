//! `GET /resolve-hash/{hash}` -- content-addressed lookup: every completed
//! build that produced a given executable hash.

use crate::{
    api::responses::ResolveHashResponse,
    db::DbClient,
    errors::{ApiError, Result},
};
use axum::{
    extract::{Path, State},
    Json,
};

pub async fn resolve(
    State(db): State<DbClient>,
    Path(hash): Path<String>,
) -> Result<Json<ResolveHashResponse>> {
    // Stored hashes are lowercase hex; reject non-hex up front (400, no query).
    let hash = hash.trim().to_lowercase();
    if hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(ApiError::BadRequest(
            "executable_hash must be a 64-character hex string".into(),
        ));
    }

    let builds = db.resolve_executable_hash(&hash).await?;
    let entries = builds.into_iter().map(Into::into).collect();

    Ok(Json(ResolveHashResponse {
        executable_hash: hash,
        builds: entries,
    }))
}
