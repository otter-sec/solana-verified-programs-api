use std::sync::Arc;

use crate::{
    db::{
        models::{VerifiedProgram, VerifiedProgramStatusResponse},
        DbClient,
    },
    services::onchain::{get_program_authority, program_metadata_retriever::get_otter_pda},
    Result, CONFIG,
};
use diesel::sql_query;
use diesel::ExpressionMethods;
use diesel::QueryDsl;
use diesel_async::RunQueryDsl;
use futures::stream::{self, StreamExt};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use tracing::{error, info};

pub const PER_PAGE: i64 = 20;

/// DbClient helper functions for VerifiedPrograms table to retrieve verified programs
impl DbClient {
    /// Retrieves all verified programs from the database
    ///
    /// Returns a list of VerifiedProgram structs
    pub async fn get_verified_programs(&self) -> Result<Vec<VerifiedProgram>> {
        let conn = &mut self.get_db_conn().await?;

        info!("Fetching distinct verified programs by program_id");

        // This query selects the first row per unique program_id based on the order of verified_at (latest first)
        let query = r#"
            SELECT DISTINCT ON (program_id) *
            FROM verified_programs
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
        use crate::schema::verified_programs::dsl::*;
        // Ensure page is valid
        let page = page.max(1);
        let offset = (page - 1) * PER_PAGE;

        let conn = &mut self.get_db_conn().await?;

        let all_program_ids = verified_programs
            .filter(is_verified.eq(true))
            .select(program_id)
            .distinct()
            .order_by(program_id)
            .load::<String>(conn)
            .await?;

        let client = Arc::new(RpcClient::new(CONFIG.rpc_url.clone()));

        // Return only programs where the verification PDA signer is the program upgrade authority, with caching for validation
        let valid_programs: Vec<String> = stream::iter(all_program_ids)
            .map(|pid| {
                let client = Arc::clone(&client);
                async move {
                    let cache_key = format!("valid_program:{}", pid);

                    // Try cache first
                    match self.get_cache(&cache_key).await {
                        Ok(cached) if cached == "1" => return Some(pid),
                        Ok(_) => return None,
                        Err(_) => {}
                    }

                    // Check DB for saved authority
                    let saved_authority = self
                        .get_program_authority_from_db(&pid)
                        .await
                        .ok()
                        .flatten();

                    // Fallback to RPC if not in DB
                    let authority = match saved_authority {
                        Some(a) => Some(a),
                        None => get_program_authority(&pid)
                            .await
                            .ok()
                            .and_then(|(auth, _)| auth),
                    };

                    if let Some(program_authority) = authority {
                        if let (Ok(program_pubkey), Ok(authority_pubkey)) = (
                            Pubkey::try_from(pid.as_str()),
                            Pubkey::try_from(program_authority.as_str()),
                        ) {
                            if get_otter_pda(&client, &authority_pubkey, &program_pubkey)
                                .await
                                .is_ok()
                            {
                                _ = self.set_cache(&cache_key, "1").await;
                                return Some(pid);
                            }
                        }
                    }

                    _ = self.set_cache(&cache_key, "0").await;
                    None
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
        let all_verified_programs = self.get_verified_programs().await?;
        let client = Arc::new(RpcClient::new(CONFIG.rpc_url.clone()));
        let this = self.clone();

        let stream = stream::iter(all_verified_programs.into_iter().map(|program| {
            let this = this.clone();
            let client = Arc::clone(&client);
            async move {
                let program_id = program.program_id.clone();

                // Try to get authority from DB first
                let saved_authority = this
                    .get_program_authority_from_db(&program_id)
                    .await
                    .ok()
                    .flatten();

                // Fallback to RPC if not found
                let authority = match saved_authority {
                    Some(a) => Some(a),
                    None => get_program_authority(&program_id)
                        .await
                        .ok()
                        .and_then(|(auth, _)| auth),
                };

                // Validate using PDA logic
                let is_valid = if let Some(program_authority) = authority {
                    if let (Ok(program_pubkey), Ok(authority_pubkey)) = (
                        Pubkey::try_from(program_id.as_str()),
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
                    return None;
                }

                match this.check_is_verified(program_id.clone(), None).await {
                    Ok(result) => {
                        let status_message = if result.is_verified {
                            "On chain program verified"
                        } else {
                            "On chain program not verified"
                        };
                        Some(VerifiedProgramStatusResponse {
                            program_id,
                            is_verified: result.is_verified,
                            message: status_message.to_string(),
                            on_chain_hash: result.on_chain_hash,
                            executable_hash: result.executable_hash,
                            last_verified_at: result.last_verified_at,
                            repo_url: result.repo_url,
                            commit: result.commit,
                        })
                    }
                    Err(err) => {
                        error!("Failed to verify program: {}", err);
                        None
                    }
                }
            }
        }))
        // Run up to 10 verification checks in parallel
        .buffer_unordered(10);

        let results: Vec<VerifiedProgramStatusResponse> =
            stream.filter_map(|res| async move { res }).collect().await;

        Ok(results)
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
