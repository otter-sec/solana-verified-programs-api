use super::models::{ProgramAuthorityParams, VerificationResponseWithSigner};
use super::DbClient;
use crate::db::models::{
    JobStatus, SolanaProgramBuild, SolanaProgramBuildParams, VerificationResponse,
    VerifiedBuildWithSigner, VerifiedProgram, DEFAULT_SIGNER,
};
use crate::services::onchain::{get_program_authority, program_metadata_retriever::SIGNER_KEYS};
use crate::services::{build_repository_url, get_on_chain_hash, onchain, verification};
use crate::Result;
use diesel::{
    expression_methods::{BoolExpressionMethods, ExpressionMethods},
    query_dsl::QueryDsl,
    sql_query, Table,
};
use diesel_async::RunQueryDsl;
use std::str::FromStr;

use solana_sdk::pubkey::Pubkey;
use tracing::{error, info, warn};

/// DbClient helper functions for VerifiedPrograms table and Reverification
impl DbClient {
    /// Check if a program is already verified
    pub async fn check_is_verified(
        &self,
        program_address: String,
        signer: Option<String>,
        authority_info: Option<ProgramAuthorityParams>,
    ) -> Result<VerificationResponse> {
        let cache_key = format!("check_is_verified:{program_address}");

        // Try to get from cache
        if let Ok(cached_str) = self.get_cache(&cache_key).await {
            if let Ok(cached) = serde_json::from_str::<VerificationResponse>(&cached_str) {
                info!("Cache hit for program {}", program_address);
                return Ok(cached);
            } else {
                warn!("Cache found but failed to deserialize, falling back...");
            }
        }

        let (res_result, build_params_result, frozen_status, closed_status) = tokio::join!(
            self.get_verified_build(&program_address, signer.clone()),
            self.get_build_params(&program_address),
            self.is_program_frozen(&program_address),
            self.is_program_closed(&program_address)
        );

        let res = res_result?;
        let build_params = build_params_result?;
        let saved_program_frozen = frozen_status?;
        let saved_program_closed = closed_status?;

        // Only fetch program authority if we don't have it provided and program is not frozen
        let (program_authority, program_frozen, program_closed) =
            if let Some(info) = &authority_info {
                (info.authority.clone(), info.frozen, info.closed)
            } else if saved_program_frozen {
                // If program is already frozen in DB, no need to check authority
                (None, true, false)
            } else {
                // Only make RPC call if program is not frozen
                get_program_authority(&program_address).await?
            };

        let return_response = |response: VerificationResponse| async {
            if let Ok(serialized) = serde_json::to_string(&response) {
                let _ = self.set_cache(&cache_key, &serialized).await;
            } else {
                warn!("Failed to serialize verification response for cache.");
            }
            Ok(response)
        };

        if let Ok(matched) = self
            .check_cache(&res.executable_hash, &program_address)
            .await
        {
            if matched {
                info!("Cache matched for program: {}", program_address);
                let response = VerificationResponse {
                    is_verified: true,
                    on_chain_hash: res.on_chain_hash,
                    executable_hash: res.executable_hash,
                    repo_url: build_repository_url(&build_params),
                    last_verified_at: Some(res.verified_at),
                    commit: build_params.commit_hash.unwrap_or_default(),
                    is_frozen: program_frozen,
                    is_closed: program_closed,
                };
                return return_response(response).await;
            }
        }

        // Update database if frozen status changed
        if program_frozen != saved_program_frozen || program_closed != saved_program_closed {
            let program_id_pubkey = Pubkey::from_str(&program_address)?;
            self.insert_or_update_program_authority(
                &program_id_pubkey,
                program_authority.as_deref(),
                program_frozen,
                Some(program_closed),
            )
            .await?;
        }

        if program_closed {
            info!("Program is closed and not verifiable.");
            let response = VerificationResponse {
                is_verified: false,
                on_chain_hash: res.on_chain_hash.clone(),
                executable_hash: res.executable_hash.clone(),
                last_verified_at: Some(res.verified_at),
                repo_url: build_params.repository.clone(),
                commit: build_params.commit_hash.unwrap_or_default(),
                is_frozen: program_frozen,
                is_closed: program_closed,
            };
            return return_response(response).await;
        }

        if program_frozen {
            info!("Program is frozen and not upgradable.");
            let response = VerificationResponse {
                is_verified: res.on_chain_hash == res.executable_hash,
                on_chain_hash: res.on_chain_hash,
                executable_hash: res.executable_hash,
                repo_url: build_repository_url(&build_params),
                last_verified_at: Some(res.verified_at),
                commit: build_params.commit_hash.unwrap_or_default(),
                is_frozen: program_frozen,
                is_closed: program_closed,
            };
            return return_response(response).await;
        }

        // Get on-chain hash and update cache
        match get_on_chain_hash(&program_address).await {
            Ok(on_chain_hash) => {
                self.set_cache(&program_address, &on_chain_hash).await?;

                if on_chain_hash != res.on_chain_hash {
                    info!("On chain hash doesn't match. Triggering re-verification.");
                    self.update_onchain_hash(
                        &program_address,
                        &on_chain_hash,
                        on_chain_hash == res.executable_hash,
                    )
                    .await?;

                    // Spawn re-verification task
                    let params_cloned = build_params.clone();
                    let this = self.clone();
                    tokio::spawn(async move {
                        let _ = this.reverify_program(params_cloned).await;
                    });
                }

                let response = VerificationResponse {
                    is_verified: on_chain_hash == res.executable_hash,
                    on_chain_hash,
                    executable_hash: res.executable_hash,
                    repo_url: build_repository_url(&build_params),
                    last_verified_at: Some(res.verified_at),
                    commit: build_params.commit_hash.unwrap_or_default(),
                    is_frozen: program_frozen,
                    is_closed: program_closed,
                };
                return return_response(response).await;
            }
            Err(e) => {
                let error_str = e.to_string();
                if error_str.contains("Program appears to be closed") {
                    info!(
                        "Program {} appears to be closed. Marking as unverified and frozen.",
                        program_address
                    );

                    // Mark the program as unverified since it's closed
                    self.mark_program_unverified(&program_address).await?;

                    // Update program authority status to frozen in database
                    let program_id_pubkey = Pubkey::from_str(&program_address)?;
                    self.insert_or_update_program_authority(
                        &program_id_pubkey,
                        None,       // No authority for closed programs
                        false,      // Don't mark as frozen
                        Some(true), // Mark as closed
                    )
                    .await?;

                    let response = VerificationResponse {
                        is_verified: false,               // Program is closed, so not verified
                        on_chain_hash: res.on_chain_hash, // Keep the last known hash
                        executable_hash: res.executable_hash,
                        repo_url: build_repository_url(&build_params),
                        last_verified_at: Some(res.verified_at),
                        commit: build_params.commit_hash.unwrap_or_default(),
                        is_frozen: false, // Don't mark as frozen, mark as closed instead
                        is_closed: true,  // Program is definitely closed in this case
                    };
                    return return_response(response).await;
                }
                info!("Failed to get on-chain hash. Using cached value.");
                let response = VerificationResponse {
                    is_verified: res.on_chain_hash == res.executable_hash,
                    on_chain_hash: res.on_chain_hash,
                    executable_hash: res.executable_hash,
                    repo_url: build_repository_url(&build_params),
                    last_verified_at: Some(res.verified_at),
                    commit: build_params.commit_hash.unwrap_or_default(),
                    is_frozen: program_frozen,
                    is_closed: program_closed,
                };
                return return_response(response).await;
            }
        }
    }

    /// Get all verification info for a program
    pub async fn get_all_verification_info(
        self,
        program_address: String,
    ) -> Result<Vec<VerificationResponseWithSigner>> {
        let cache_key = format!("get_all_verification_info:{program_address}");

        // Try fetching from cache
        if let Ok(cached_str) = self.get_cache(&cache_key).await {
            if let Ok(cached_data) =
                serde_json::from_str::<Vec<VerificationResponseWithSigner>>(&cached_str)
            {
                info!("Cache hit for all verification info: {}", program_address);
                return Ok(cached_data);
            } else {
                warn!("Cache found for all verification info but failed to deserialize, proceeding...");
            }
        }

        let res = self
            .get_verified_builds_with_signer(&program_address)
            .await?;

        let hash = match self.get_cache(&program_address).await {
            Ok(cache_result) => Some(cache_result),
            Err(_) => {
                match get_on_chain_hash(&program_address).await {
                    Ok(on_chain_hash) => {
                        self.set_cache(&program_address, &on_chain_hash).await.ok();
                        Some(on_chain_hash)
                    }
                    Err(e) => {
                        let error_str = e.to_string();
                        if error_str.contains("Program appears to be closed") {
                            info!("Program {} appears to be closed during get_all_verification_info. Marking as unverified and frozen.", program_address);

                            // Mark all builds for this program as unverified
                            self.mark_program_unverified(&program_address).await.ok();

                            // Update program authority status to frozen in database
                            if let Ok(program_id_pubkey) = Pubkey::from_str(&program_address) {
                                self.insert_or_update_program_authority(
                                    &program_id_pubkey,
                                    None,       // No authority for closed programs
                                    false,      // Don't mark as frozen
                                    Some(true), // Mark as closed
                                )
                                .await
                                .ok();
                            }
                        }
                        None
                    }
                }
            }
        };

        let mut is_verification_needed = false;
        let mut verification_responses = vec![];

        let mut is_frozen_status_update_needed = false;
        let mut is_frozen_status_update_data = ProgramAuthorityParams {
            authority: None,
            frozen: false,
            closed: false,
        };

        // Track if we've already updated the on-chain hash for this program
        let mut program_on_chain_hash_updated = false;

        // Process each build individually
        for verified_build_with_signer in res {
            let build = verified_build_with_signer.solana_program_build;
            let verified_build = verified_build_with_signer.verified_program;
            let mut is_program_frozen;

            if let Some(verified_build) = verified_build {
                // Check if on-chain hash has changed once per program, not per build
                // Since we are updating the on-chain hash for all builds of a program at once
                if let Some(ref fresh_on_chain_hash) = hash {
                    if !program_on_chain_hash_updated {
                        let stored_on_chain_hash = &verified_build.on_chain_hash;
                        if fresh_on_chain_hash != stored_on_chain_hash {
                            info!(
                                "On-chain hash changed from {} to {}. Updating all builds for program.",
                                stored_on_chain_hash, fresh_on_chain_hash
                            );
                            self.update_program_onchain_hash(&program_address, fresh_on_chain_hash)
                                .await?;
                            program_on_chain_hash_updated = true;
                            is_verification_needed = true;
                        }
                    }
                }

                // Determine if this specific build is currently verified
                let is_verified = if let Some(ref fresh_on_chain_hash) = hash {
                    // Build is verified if current on-chain hash matches this build's executable hash
                    *fresh_on_chain_hash == verified_build.executable_hash
                } else {
                    // No current on-chain hash available, compare stored hashes
                    verified_build.executable_hash == verified_build.on_chain_hash
                };

                is_program_frozen = verified_build_with_signer.is_frozen.unwrap_or_default();

                if !is_program_frozen {
                    let (current_authority, current_frozen_status, _current_closed_status) =
                        get_program_authority(&program_address)
                            .await
                            .unwrap_or((None, false, false));

                    if current_frozen_status != is_program_frozen {
                        is_frozen_status_update_needed = true;
                        is_frozen_status_update_data.authority = current_authority;
                        is_frozen_status_update_data.frozen = current_frozen_status;
                    }

                    is_program_frozen = current_frozen_status;
                }

                verification_responses.push(VerificationResponseWithSigner {
                    verification_response: VerificationResponse {
                        is_verified,
                        on_chain_hash: verified_build.on_chain_hash,
                        executable_hash: verified_build.executable_hash,
                        repo_url: build_repository_url(&build),
                        last_verified_at: Some(verified_build.verified_at),
                        commit: build.commit_hash.unwrap_or_default(),
                        is_frozen: is_program_frozen,
                        is_closed: false, // Default to false for existing verified programs
                    },
                    signer: build.signer.unwrap_or(DEFAULT_SIGNER.to_string()),
                });
            }
        }

        if is_frozen_status_update_needed {
            let program_id_pubkey = Pubkey::from_str(&program_address)?;
            self.insert_or_update_program_authority(
                &program_id_pubkey,
                is_frozen_status_update_data.authority.as_deref(),
                is_frozen_status_update_data.frozen,
                Some(is_frozen_status_update_data.closed),
            )
            .await?;
        }

        if is_verification_needed {
            let params = self.get_build_params(&program_address).await?;
            let this = self.clone();
            tokio::spawn(async move {
                let _ = this.reverify_program(params).await;
            });
        }

        // Cache the result
        if let Ok(serialized) = serde_json::to_string(&verification_responses) {
            self.set_cache(&cache_key, &serialized).await.ok();
        }

        Ok(verification_responses)
    }

    /// Get the verification status for a program
    ///
    /// Returns a VerifiedProgram struct
    pub async fn get_verified_build(
        &self,
        program_address: &str,
        signer: Option<String>,
    ) -> Result<VerifiedProgram> {
        use crate::schema::verified_programs::dsl::*;

        info!("Getting verified build for {:?}", program_address);
        let conn = &mut self.get_db_conn().await?;

        let query = verified_programs
            .left_join(crate::schema::solana_program_builds::table)
            .filter(program_id.eq(program_address))
            .select(verified_programs::all_columns())
            .order((is_verified.desc(), verified_at.desc()));

        match signer {
            Some(signer) => query
                .filter(crate::schema::solana_program_builds::signer.eq(signer))
                .first::<VerifiedProgram>(conn)
                .await
                .map_err(|e| {
                    error!("Failed to get solana_program_builds: {}", e);
                    e.into()
                }),
            None => {
                let program_authority = self.get_program_authority_from_db(program_address).await;
                let mut filtered_query = query
                    .filter(
                        crate::schema::solana_program_builds::signer
                            .eq(Some(DEFAULT_SIGNER.to_string()))
                            .or(crate::schema::solana_program_builds::signer
                                .eq(Some(SIGNER_KEYS[0].to_string())))
                            .or(crate::schema::solana_program_builds::signer
                                .eq(Some(SIGNER_KEYS[1].to_string())))
                            .or(crate::schema::solana_program_builds::signer
                                .eq(Some(SIGNER_KEYS[2].to_string())))
                            .or(crate::schema::solana_program_builds::signer.is_null()),
                    )
                    .into_boxed();

                if let Ok(Some(program_authority)) = program_authority {
                    filtered_query = query
                        .filter(
                            crate::schema::solana_program_builds::signer
                                .eq(Some(DEFAULT_SIGNER.to_string()))
                                .or(crate::schema::solana_program_builds::signer
                                    .eq(Some(SIGNER_KEYS[0].to_string())))
                                .or(crate::schema::solana_program_builds::signer
                                    .eq(Some(SIGNER_KEYS[1].to_string())))
                                .or(crate::schema::solana_program_builds::signer
                                    .eq(Some(SIGNER_KEYS[2].to_string())))
                                .or(crate::schema::solana_program_builds::signer
                                    .eq(Some(program_authority)))
                                .or(crate::schema::solana_program_builds::signer.is_null()),
                        )
                        .into_boxed();
                }

                filtered_query
                    .first::<VerifiedProgram>(conn)
                    .await
                    .map_err(|e| {
                        error!("Failed to get verified program data: {}", e);
                        e.into()
                    })
            }
        }
    }

    pub async fn get_verified_builds_with_signer(
        &self,
        program_address: &str,
    ) -> Result<Vec<VerifiedBuildWithSigner>> {
        let conn = &mut self.get_db_conn().await?;
        sql_query(
            r#"
            SELECT
                *
            FROM
                (
                    SELECT
                        *,
                        ROW_NUMBER() OVER (
                            PARTITION BY
                                COALESCE(sp.signer, '11111111111111111111111111111111')
                            ORDER BY
                                created_at
                        ) AS rn
                    FROM
                        verified_programs vp
                        LEFT JOIN solana_program_builds sp ON sp.id = vp.solana_build_id
                        LEFT JOIN program_authority pa ON pa.program_id = $1
                    WHERE
                        vp.program_id = $1 AND vp.is_verified = true
                ) subquery
            WHERE
                rn = 1
        "#,
        )
        .bind::<diesel::sql_types::Text, _>(program_address)
        .load::<VerifiedBuildWithSigner>(conn)
        .await
        .map_err(|e| {
            error!("Failed to get verified builds with signer: {}", e);
            e.into()
        })
    }

    /// Insert or update a verified program
    pub async fn insert_or_update_verified_build(
        &self,
        payload: &VerifiedProgram,
    ) -> Result<usize> {
        use crate::schema::verified_programs::dsl::*;

        let conn = &mut self.get_db_conn().await?;
        diesel::insert_into(verified_programs)
            .values(payload)
            .on_conflict(id)
            .do_update()
            .set(payload)
            .execute(conn)
            .await
            .map_err(|e| {
                error!("Failed to insert/update verified build: {}", e);
                e.into()
            })
    }

    /// Update the on-chain hash for a program
    pub async fn update_onchain_hash(
        &self,
        program_address: &str,
        on_chainhash: &str,
        isverified: bool,
    ) -> Result<usize> {
        use crate::schema::verified_programs::dsl::*;

        let conn = &mut self.get_db_conn().await?;

        diesel::update(verified_programs)
            .filter(program_id.eq(program_address))
            .set((
                on_chain_hash.eq(on_chainhash),
                is_verified.eq(isverified),
                verified_at.eq(chrono::Utc::now().naive_utc()),
            ))
            .execute(conn)
            .await
            .map_err(|e| {
                error!("Failed to update on-chain hash: {}", e);
                e.into()
            })
    }

    /// Update the on-chain hash and verification status for all builds of a program
    /// Sets is_verified based on whether executable_hash matches the new on-chain hash
    pub async fn update_program_onchain_hash(
        &self,
        program_address: &str,
        new_onchain_hash: &str,
    ) -> Result<usize> {
        let conn = &mut self.get_db_conn().await?;

        sql_query(
            r#"
            UPDATE verified_programs
            SET
                on_chain_hash = $1,
                is_verified = (executable_hash = $1),
                verified_at = NOW()
            WHERE program_id = $2
            "#,
        )
        .bind::<diesel::sql_types::Text, _>(new_onchain_hash)
        .bind::<diesel::sql_types::Text, _>(program_address)
        .execute(conn)
        .await
        .map_err(|e| {
            error!("Failed to update program on-chain hash: {}", e);
            e.into()
        })
    }

    /// Re-verify a program using on-chain metadata
    pub async fn reverify_program(self, build_params: SolanaProgramBuild) {
        info!("Re-verifying the build.");
        let mut payload = SolanaProgramBuildParams {
            program_id: build_params.program_id,
            repository: build_params.repository,
            commit_hash: build_params.commit_hash,
            lib_name: build_params.lib_name,
            base_image: build_params.base_docker_image,
            mount_path: build_params.mount_path,
            bpf_flag: Some(build_params.bpf_flag),
            cargo_args: build_params.cargo_args,
            arch: build_params.arch,
        };

        // Better error handling for program authority
        let (program_authority, is_frozen, is_closed) =
            match get_program_authority(&payload.program_id).await {
                Ok(authority) => authority,
                Err(e) => {
                    error!(
                        "Failed to get program authority for {}: {:?}",
                        payload.program_id, e
                    );
                    (None, false, false)
                }
            };

        let params_from_onchain =
            onchain::get_otter_verify_params(&payload.program_id, None, program_authority.clone())
                .await;

        if let Ok((params_from_onchain, _)) = params_from_onchain {
            // Store program authority in database if available
            if let Err(e) = self
                .insert_or_update_program_authority(
                    &params_from_onchain.address,
                    program_authority.as_deref(),
                    is_frozen,
                    Some(is_closed),
                )
                .await
            {
                error!(
                    "Failed to update program authority for {}: {:?}",
                    params_from_onchain.address, e
                );
            }

            let otter_params = SolanaProgramBuildParams::from(params_from_onchain);
            if otter_params != payload {
                info!("Build params from on-chain and database don't match. Re-verifying the build using onchain Metadata.");
                payload = otter_params;
            }
        } else if let Err(e) = params_from_onchain {
            error!(
                "Failed to get on-chain parameters for {}: {:?}",
                payload.program_id, e
            );
        }

        let build_id = build_params.id;
        let random_file_id = uuid::Uuid::new_v4().to_string();

        tokio::spawn(async move {
            match verification::execute_verification(payload, &build_id, &random_file_id).await {
                Ok(res) => {
                    if let Err(e) = self.insert_or_update_verified_build(&res).await {
                        error!("Failed to insert/update verified build: {:?}", e);
                    }
                    if let Err(e) = self
                        .update_build_status(&build_id, JobStatus::Completed)
                        .await
                    {
                        error!("Failed to update build status to completed: {:?}", e);
                    }
                }
                Err(err) => {
                    if let Err(e) = self.update_build_status(&build_id, JobStatus::Failed).await {
                        error!("Failed to update build status to failed: {:?}", e);
                    }
                    error!("Error verifying build: {:?}", err);
                }
            }
        });
    }

    /// Unverify a program by updating the on-chain hash
    pub async fn unverify_program(
        &self,
        program_address: &str,
        on_chainhash: &str,
    ) -> Result<usize> {
        use crate::schema::verified_programs::dsl::*;

        let conn = &mut self.get_db_conn().await?;

        diesel::update(verified_programs)
            .filter(program_id.eq(program_address))
            .set((
                on_chain_hash.eq(on_chainhash),
                is_verified.eq(false),
                verified_at.eq(chrono::Utc::now().naive_utc()),
            ))
            .execute(conn)
            .await
            .map_err(|e| {
                error!("Failed to unverify program: {}", e);
                e.into()
            })
    }
    /// Mark a program as unverified without modifying the on-chain hash.
    pub async fn mark_program_unverified(&self, program_address: &str) -> Result<usize> {
        use crate::schema::verified_programs::dsl::*;

        let conn = &mut self.get_db_conn().await?;

        diesel::update(verified_programs)
            .filter(program_id.eq(program_address))
            .set((
                is_verified.eq(false),
                verified_at.eq(chrono::Utc::now().naive_utc()),
            ))
            .execute(conn)
            .await
            .map_err(|e| {
                error!("Failed to mark program as unverified: {}", e);
                e.into()
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::models::ProgramAuthorityParams;

    #[tokio::test]
    async fn test_verification_response_includes_closed_status() {
        // Test that VerificationResponse properly includes both is_frozen and is_closed fields

        let test_response = VerificationResponse {
            is_verified: true,
            on_chain_hash: "test_hash".to_string(),
            executable_hash: "test_hash".to_string(),
            repo_url: "https://github.com/test/repo".to_string(),
            commit: "abcd1234".to_string(),
            last_verified_at: Some(chrono::Utc::now().naive_utc()),
            is_frozen: false,
            is_closed: true,
        };

        // Verify all fields are accessible
        assert!(test_response.is_verified);
        assert!(!test_response.is_frozen);
        assert!(test_response.is_closed);
        assert_eq!(test_response.repo_url, "https://github.com/test/repo");

        // Test serialization/deserialization
        let serialized = serde_json::to_string(&test_response).expect("Should serialize");
        let deserialized: VerificationResponse =
            serde_json::from_str(&serialized).expect("Should deserialize");

        assert_eq!(test_response.is_frozen, deserialized.is_frozen);
        assert_eq!(test_response.is_closed, deserialized.is_closed);
        assert_eq!(test_response.is_verified, deserialized.is_verified);
    }

    #[test]
    fn test_program_authority_params_with_closed_status() {
        // Test that ProgramAuthorityParams includes closed field
        let params = ProgramAuthorityParams {
            authority: Some("test_authority".to_string()),
            frozen: true,
            closed: false,
        };

        assert_eq!(params.authority, Some("test_authority".to_string()));
        assert!(params.frozen);
        assert!(!params.closed);

        let closed_params = ProgramAuthorityParams {
            authority: None,
            frozen: false,
            closed: true,
        };

        assert_eq!(closed_params.authority, None);
        assert!(!closed_params.frozen);
        assert!(closed_params.closed);
    }

    #[test]
    fn test_verification_response_with_signer_includes_closed_status() {
        // Test that VerificationResponseWithSigner properly includes closed status
        let verification_response = VerificationResponse {
            is_verified: false,
            on_chain_hash: "hash1".to_string(),
            executable_hash: "hash2".to_string(),
            repo_url: "https://github.com/example/repo".to_string(),
            commit: "commit123".to_string(),
            last_verified_at: None,
            is_frozen: true,
            is_closed: true,
        };

        let response_with_signer = VerificationResponseWithSigner {
            verification_response,
            signer: "test_signer".to_string(),
        };

        // Verify the nested verification response has the closed status
        assert!(response_with_signer.verification_response.is_frozen);
        assert!(response_with_signer.verification_response.is_closed);
        assert!(!response_with_signer.verification_response.is_verified);
        assert_eq!(response_with_signer.signer, "test_signer");
    }
}
