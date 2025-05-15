use std::{str::FromStr, sync::Arc};

use crate::{
    db::{
        models::{VerifiedProgram, VerifiedProgramStatusResponse},
        DbClient,
    },
    services::onchain::{get_program_authority, program_metadata_retriever::get_otter_pda},
    Result, CONFIG,
};
use diesel::sql_query;
use diesel_async::RunQueryDsl;
use futures::stream::{self, StreamExt};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use tracing::{error, info, warn};

use super::models::{ProgramAuthorityParams, VerificationResponse};

pub const PER_PAGE: i64 = 20;

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

        let all_verified_programs = self.get_verified_programs().await?;
        let all_program_ids: Vec<String> = all_verified_programs
            .into_iter()
            .map(|p| p.program_id)
            .collect();

        let client = Arc::new(RpcClient::new(CONFIG.rpc_url.clone()));

        let this = Arc::new(self.clone());

        // Return only programs where the verification PDA signer is the program upgrade authority, with caching for validation
        let valid_programs: Vec<String> = stream::iter(all_program_ids)
            .map(|pid| {
                let client = Arc::clone(&client);
                let this = Arc::clone(&this);
                async move {
                    match this
                        .is_program_valid_and_verified(&pid, client.clone())
                        .await
                    {
                        Ok(Some(_)) => Some(pid), // Valid and verified
                        _ => None,                // Invalid or error
                    }
                }
            })
            .buffer_unordered(10)
            .filter_map(|x| async move { x })
            .collect()
            .await;

        let total_count = valid_programs.len() as i64;

        let paginated_programs = valid_programs
            .into_iter()
            .skip(offset as usize)
            .take(PER_PAGE as usize)
            .collect();

        Ok((paginated_programs, total_count))
    }

    pub async fn get_verification_status_all(&self) -> Result<Vec<VerifiedProgramStatusResponse>> {
        let all_verified_programs: Vec<VerifiedProgram> = self.get_verified_programs().await?;
        let client = Arc::new(RpcClient::new(CONFIG.rpc_url.clone()));
        let this = Arc::new(self.clone());

        let stream = stream::iter(all_verified_programs.into_iter().map(|program| {
            let this = Arc::clone(&this);
            let client = Arc::clone(&client);
            async move {
                let program_id = program.program_id.clone();

                match this
                    .clone()
                    .is_program_valid_and_verified(&program_id, client.clone())
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
        .buffer_unordered(10);

        let results: Vec<VerifiedProgramStatusResponse> =
            stream.filter_map(|res| async move { res }).collect().await;

        Ok(results)
    }

    pub async fn is_program_valid_and_verified(
        self: Arc<Self>,
        program_id: &str,
        client: Arc<RpcClient>,
    ) -> Result<Option<VerificationResponse>> {
        let cache_key = format!("is_program_valid_and_verified:{}", program_id);

        // Try to get from cache
        if let Ok(cached_str) = self.get_cache(&cache_key).await {
            if cached_str == "NOT_VERIFIED" {
                info!("Cache hit (NOT_VERIFIED) for program {}", program_id);
                return Ok(None);
            }
            if let Ok(cached) = serde_json::from_str::<VerificationResponse>(&cached_str) {
                info!("Cache hit for program {}", program_id);
                return Ok(Some(cached));
            } else {
                warn!("Cache found but failed to deserialize, falling back...");
            }
        }
        // Saving get_program_authority result and passing to check_is_verified
        // To avoid multiple time
        let mut onchain_authoriry: Option<ProgramAuthorityParams> = None;

        let mut authority = self
            .get_program_authority_from_db(program_id)
            .await
            .ok()
            .flatten();

        // Fetch and save authority if not in DB
        if authority.is_none() {
            if let Ok((auth_opt, frozen)) = get_program_authority(program_id).await {
                authority = auth_opt.clone();
                onchain_authoriry = Some(ProgramAuthorityParams {
                    authority: auth_opt.clone(),
                    frozen,
                });
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

        let is_valid = if let Some(program_authority) = authority {
            if let (Ok(program_pubkey), Ok(authority_pubkey)) = (
                Pubkey::try_from(program_id),
                Pubkey::try_from(program_authority.as_str()),
            ) {
                get_otter_pda(&client, &authority_pubkey, &program_pubkey)
                    .await
                    .is_ok()
            } else {
                false
            }
        } else {
            false
        };

        if !is_valid {
            warn!("Invalid program: {}", program_id);
            let _ = self.set_cache(&cache_key, "NOT_VERIFIED").await;
            return Ok(None);
        }

        match self
            .check_is_verified(program_id.to_string(), None, onchain_authoriry)
            .await
        {
            Ok(res) if res.is_verified => {
                if let Ok(serialized) = serde_json::to_string(&res) {
                    let _ = self.set_cache(&cache_key, &serialized).await;
                } else {
                    warn!("Failed to serialize verification response for cache.");
                }
                Ok(Some(res))
            }
            Ok(_) => {
                warn!("Program {} is not verified", program_id);
                let _ = self.set_cache(&cache_key, "NOT_VERIFIED").await;
                Ok(None)
            }
            Err(e) => {
                error!("Verification error for {}: {}", program_id, e);
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
