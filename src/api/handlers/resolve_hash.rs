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
    let hash = hash.trim().to_string();
    if hash.is_empty() {
        return Err(ApiError::BadRequest("hash cannot be empty".into()));
    }

    let builds = db.builds_by_executable_hash(&hash).await?;
    let mut entries = Vec::with_capacity(builds.len());
    for b in builds {
        let on_chain_hash = db
            .get_program_state(&b.program_id)
            .await
            .ok()
            .flatten()
            .and_then(|s| s.on_chain_hash);
        let matches_deployed = on_chain_hash.as_deref() == Some(hash.as_str());

        entries.push(ResolveHashEntry {
            build_id: b.id.to_string(),
            program_id: b.program_id,
            signer: b.signer,
            repository: b.repository,
            commit: b.commit_hash,
            completed_at: b.completed_at.map(|t| t.naive_utc()),
            matches_deployed,
        });
    }

    Ok(Json(ResolveHashResponse {
        executable_hash: hash,
        builds: entries,
    }))
}
