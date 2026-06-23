//! `GET /resolve-hash/{hash}` -- content-addressed lookup: every completed
//! build that produced a given executable hash.

use crate::{
    api::responses::{ResolveHashEntry, ResolveHashResponse},
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
    let entries = builds
        .into_iter()
        .map(|b| ResolveHashEntry {
            build_id: b.id.to_string(),
            program_id: b.program_id,
            signer: b.signer,
            repository: b.repository,
            commit: b.commit_hash,
            completed_at: b.completed_at.map(|t| t.naive_utc()),
            matches_deployed: b.matches_deployed,
        })
        .collect();

    Ok(Json(ResolveHashResponse {
        executable_hash: hash,
        builds: entries,
    }))
}
