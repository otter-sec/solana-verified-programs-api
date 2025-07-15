use std::{str::FromStr, sync::Arc};

use crate::{
    db::{
        models::{VerifiedProgram, VerifiedProgramStatusResponse},
        redis::VERIFIED_PROGRAM_CACHE_EXPIRY_SECONDS,
        DbClient,
    },
    services::onchain::{get_program_authority, program_metadata_retriever::get_otter_pda},
    Result, CONFIG,
};
use diesel::{sql_query, sql_types::BigInt, QueryableByName};
use diesel_async::RunQueryDsl;
use futures::stream::{self, StreamExt};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use tracing::{error, info};

use super::models::{ProgramAuthorityParams, VerificationResponse};

pub const PER_PAGE: i64 = 20;

#[derive(QueryableByName)]
struct CountResult {
    #[diesel(sql_type = BigInt)]
    total: i64,
}

#[derive(QueryableByName)]
struct ProgramIdResult {
    #[diesel(sql_type = diesel::sql_types::Text)]
    program_id: String,
}

/// DbClient helper functions for VerifiedPrograms table to retrieve verified programs
impl DbClient {
    /// Retrieves all verified programs from the database
    ///
    /// Returns a list of VerifiedProgram structs
    pub async fn get_verified_programs(&self) -> Result<Vec<VerifiedProgram>> {
        let conn = &mut self.get_db_conn().await?;

        info!("Fetching distinct verified programs by program_id");

        // Select only verified rows and get the latest per program_id
        let query = r#"
            SELECT DISTINCT ON (program_id) *
            FROM verified_programs
            WHERE is_verified = true
            ORDER BY program_id, verified_at DESC
        "#;

        sql_query(query)
            .load::<VerifiedProgram>(conn)
            .await
            .map_err(|e| {
                error!("Failed to fetch distinct verified programs: {}", e);
                e.into()
            })
    }

    /// Retrieves a page of verified programs from the database
    ///
    /// Returns a list of VerifiedProgram structs
    ///
    ///  
    pub async fn get_verified_program_ids_page(&self, page: i64) -> Result<(Vec<String>, i64)> {
        // Ensure page is valid
        let page = page.max(1);
        let offset = (page - 1) * PER_PAGE;

        // Use a single query to get verified programs with pagination
        let conn = &mut self.get_db_conn().await?;

        // First get the total count of verified programs
        let count_query = r#"
            SELECT COUNT(DISTINCT program_id) as total
            FROM verified_programs
            WHERE is_verified = true
        "#;

        let total_count: i64 = sql_query(count_query)
            .get_result::<CountResult>(conn)
            .await
            .map_err(|e| {
                error!("Failed to get total count of verified programs: {}", e);
                e
            })?
            .total;

        // Get paginated verified programs
        let query = r#"
            SELECT DISTINCT program_id
            FROM verified_programs
            WHERE is_verified = true
            ORDER BY program_id
            LIMIT $1 OFFSET $2
        "#;

        let program_ids: Vec<String> = sql_query(query)
            .bind::<diesel::sql_types::BigInt, _>(PER_PAGE)
            .bind::<diesel::sql_types::BigInt, _>(offset)
            .get_results::<ProgramIdResult>(conn)
            .await
            .map_err(|e| {
                error!("Failed to fetch paginated verified programs: {}", e);
                e
            })?
            .into_iter()
            .map(|result| result.program_id)
            .collect();

        // Now validate programs in batches with proper concurrency control
        let client = Arc::new(RpcClient::new(CONFIG.rpc_url.clone()));
        let this = self.clone();

        // Use a semaphore to limit concurrent RPC calls and prevent overwhelming the RPC endpoint
        let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(10));

        let valid_programs: Vec<String> = stream::iter(program_ids)
            .map(|pid| {
                let client = Arc::clone(&client);
                let this = this.clone();
                let semaphore = Arc::clone(&semaphore);
                async move {
                    // Acquire semaphore permit before making RPC calls
                    let _permit = semaphore
                        .acquire()
                        .await
                        .expect("Semaphore should not be closed");

                    match this.is_program_valid_and_verified(&pid, client).await {
                        Ok(Some(_)) => Some(pid), // Valid and verified
                        _ => None,                // Invalid or error
                    }
                }
            })
            .buffer_unordered(25) // Allow more tasks to be queued while respecting semaphore limit
            .filter_map(|x| async move { x })
            .collect()
            .await;

        Ok((valid_programs, total_count))
    }

    pub async fn get_verification_status_all(&self) -> Result<Vec<VerifiedProgramStatusResponse>> {
        let all_verified_programs: Vec<VerifiedProgram> = self.get_verified_programs().await?;
        let client = Arc::new(RpcClient::new(CONFIG.rpc_url.clone()));
        let this = self.clone();

        // Use semaphore to control concurrent RPC calls
        let semaphore = std::sync::Arc::new(tokio::sync::Semaphore::new(10));

        let stream = stream::iter(all_verified_programs.into_iter().map(|program| {
            let this = this.clone();
            let client = Arc::clone(&client);
            let semaphore = Arc::clone(&semaphore);
            async move {
                let program_id = program.program_id.clone();

                // Acquire semaphore permit before making RPC calls
                let _permit = semaphore
                    .acquire()
                    .await
                    .expect("Semaphore should not be closed");

                match this
                    .is_program_valid_and_verified(&program_id, client)
                    .await
                {
                    Ok(Some(result)) => Some(VerifiedProgramStatusResponse {
                        program_id,
                        is_verified: result.is_verified,
                        message: "On chain program verified".to_string(),
                        on_chain_hash: result.on_chain_hash,
                        executable_hash: result.executable_hash,
                        last_verified_at: result.last_verified_at,
                        repo_url: result.repo_url,
                        commit: result.commit,
                    }),
                    Ok(None) => None,
                    Err(err) => {
                        error!("Failed to verify program {}: {}", program_id, err);
                        None
                    }
                }
            }
        }))
        .buffer_unordered(20);

        let results: Vec<VerifiedProgramStatusResponse> =
            stream.filter_map(|res| async move { res }).collect().await;

        Ok(results)
    }

    pub async fn is_program_valid_and_verified(
        &self,
        program_id: &str,
        client: Arc<RpcClient>,
    ) -> Result<Option<VerificationResponse>> {
        let cache_key = format!("is_program_valid_and_verified:{}", program_id);

        // Try to get from cache first
        if let Ok(cached_str) = self.get_cache(&cache_key).await {
            if cached_str == "NOT_VERIFIED" {
                return Ok(None);
            }
            if let Ok(cached) = serde_json::from_str::<VerificationResponse>(&cached_str) {
                return Ok(Some(cached));
            }
        }

        // Authority lookup
        let mut authority = self
            .get_program_authority_from_db(program_id)
            .await
            .ok()
            .flatten();

        let mut onchain_authority: Option<ProgramAuthorityParams> = None;

        // Only fetch from chain if not in DB cache
        if authority.is_none() {
            if let Ok((auth_opt, frozen)) = get_program_authority(program_id).await {
                authority = auth_opt.clone();
                onchain_authority = Some(ProgramAuthorityParams {
                    authority: auth_opt.clone(),
                    frozen,
                });

                // Insert authority data
                if frozen {
                    if let Ok(program_pubkey) = Pubkey::from_str(program_id) {
                        let _ = self
                            .insert_or_update_program_authority(
                                &program_pubkey,
                                auth_opt.as_deref(),
                                frozen,
                            )
                            .await;
                    }
                }
            }
        }

        // Handle programs with no authority (frozen/immutable programs)
        let program_authority = match authority {
            Some(auth) => auth,
            None => {
                // For programs with no authority, check if they have verified builds
                match self
                    .check_is_verified(program_id.to_string(), None, onchain_authority)
                    .await
                {
                    Ok(res) if res.is_verified => {
                        if let Ok(serialized) = serde_json::to_string(&res) {
                            let _ = self
                                .set_cache_with_expiry(
                                    &cache_key,
                                    &serialized,
                                    VERIFIED_PROGRAM_CACHE_EXPIRY_SECONDS,
                                )
                                .await;
                        }
                        return Ok(Some(res));
                    }
                    _ => {
                        let _ = self.set_cache(&cache_key, "NOT_VERIFIED").await;
                        return Ok(None);
                    }
                }
            }
        };

        // Validate program authority efficiently
        let is_valid = match (
            Pubkey::try_from(program_id),
            Pubkey::try_from(program_authority.as_str()),
        ) {
            (Ok(program_pubkey), Ok(authority_pubkey)) => {
                get_otter_pda(&client, &authority_pubkey, &program_pubkey)
                    .await
                    .is_ok()
            }
            _ => false,
        };

        if !is_valid {
            let _ = self.set_cache(&cache_key, "NOT_VERIFIED").await;
            return Ok(None);
        }

        // Check verification status
        match self
            .check_is_verified(program_id.to_string(), None, onchain_authority)
            .await
        {
            Ok(res) if res.is_verified => {
                if let Ok(serialized) = serde_json::to_string(&res) {
                    let _ = self
                        .set_cache_with_expiry(
                            &cache_key,
                            &serialized,
                            VERIFIED_PROGRAM_CACHE_EXPIRY_SECONDS,
                        )
                        .await;
                }
                Ok(Some(res))
            }
            Ok(_) => {
                let _ = self.set_cache(&cache_key, "NOT_VERIFIED").await;
                Ok(None)
            }
            Err(_) => {
                let _ = self.set_cache(&cache_key, "NOT_VERIFIED").await;
                Ok(None)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_verified_programs() {
        dotenv::dotenv().ok();
        let db_url = std::env::var("TEST_DATABASE_URL").unwrap();
        let redis_url = std::env::var("TEST_REDIS_URL").unwrap();
        let client = DbClient::new(&db_url, &redis_url);

        let result = client.get_verified_programs().await;
        assert!(result.is_ok());
    }
}
