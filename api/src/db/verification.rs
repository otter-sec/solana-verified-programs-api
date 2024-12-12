use super::models::VerificationResponseWithSigner;
use super::DbClient;
use crate::db::models::{
    JobStatus, SolanaProgramBuild, SolanaProgramBuildParams, VerificationResponse, VerifiedProgram,
    DEFAULT_SIGNER,
};
use crate::services::onchain::{get_program_authority, program_metadata_retriever::SIGNER_KEYS};
use crate::services::{get_on_chain_hash, build_repository_url, onchain, verification};
use crate::Result;
use diesel::{
    expression_methods::{BoolExpressionMethods, ExpressionMethods},
    query_dsl::QueryDsl,
    sql_query, Table,
};
use diesel_async::RunQueryDsl;

use tracing::{error, info};

/// DbClient helper functions for VerifiedPrograms table and Reverification 
impl DbClient {
    /// Check if a program is already verified
    pub async fn check_is_verified(
        self,
        program_address: String,
        signer: Option<String>,
    ) -> Result<VerificationResponse> {
        let res = self.get_verified_build(&program_address, signer).await?;
        let build_params = self.get_build_params(&program_address).await?;

        // Check cache first
        if let Ok(matched) = self
            .check_cache(&res.executable_hash, &program_address)
            .await
        {
            if matched {
                info!("Cache matched for program: {}", program_address);
                return Ok(VerificationResponse {
                    is_verified: true,
                    on_chain_hash: res.on_chain_hash,
                    executable_hash: res.executable_hash,
                    repo_url: build_repository_url(&build_params),
                    last_verified_at: Some(res.verified_at),
                    commit: build_params.commit_hash.unwrap_or_default(),
                });
            }
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
                    tokio::spawn(async move {
                        self.reverify_program(params_cloned).await;
                    });
                }

                Ok(VerificationResponse {
                    is_verified: on_chain_hash == res.executable_hash,
                    on_chain_hash,
                    executable_hash: res.executable_hash,
                    repo_url: build_repository_url(&build_params),
                    last_verified_at: Some(res.verified_at),
                    commit: build_params.commit_hash.unwrap_or_default(),
                })
            }
            Err(_) => {
                info!("Failed to get on-chain hash. Using cached value.");
                Ok(VerificationResponse {
                    is_verified: res.on_chain_hash == res.executable_hash,
                    on_chain_hash: res.on_chain_hash,
                    executable_hash: res.executable_hash,
                    repo_url: build_repository_url(&build_params),
                    last_verified_at: Some(res.verified_at),
                    commit: build_params.commit_hash.unwrap_or_default(),
                })
            }
        }
    }

    /// Get all verification info for a program
    pub async fn get_all_verification_info(
        self,
        program_address: String,
    ) -> Result<Vec<VerificationResponseWithSigner>> {
        let res = self
            .get_verified_builds_with_signer(&program_address)
            .await?;

        let hash = if let Ok(cache_result) = self.get_cache(&program_address).await {
            Some(cache_result)
        } else {
            match get_on_chain_hash(&program_address).await {
                Ok(on_chain_hash) => {
                    self.set_cache(&program_address, &on_chain_hash).await?;
                    Some(on_chain_hash)
                }
                Err(_) => None,
            }
        };

        let mut is_verification_needed = false;
        let mut verification_responses = vec![];

        for (build, verified_build) in res {
            if let Some(verified_build) = verified_build {
                let is_verified = if let Some(ref hash) = hash {
                    if *hash != verified_build.executable_hash {
                        info!("On chain hash doesn't match.");
                        self.update_onchain_hash(
                            &program_address,
                            hash,
                            *hash == verified_build.executable_hash,
                        )
                        .await?;
                        is_verification_needed = true;
                    }
                    *hash == verified_build.executable_hash
                } else {
                    verified_build.executable_hash == verified_build.on_chain_hash
                };

                verification_responses.push(VerificationResponseWithSigner {
                    verification_response: VerificationResponse {
                        is_verified,
                        on_chain_hash: verified_build.on_chain_hash,
                        executable_hash: verified_build.executable_hash,
                        repo_url: build_repository_url(&build),
                        last_verified_at: Some(verified_build.verified_at),
                        commit: build.commit_hash.unwrap_or_default(),
                    },
                    signer: build.signer.unwrap_or(DEFAULT_SIGNER.to_string()),
                });
            }
        }

        if is_verification_needed {
            let params = self.get_build_params(&program_address).await?;
            tokio::spawn(async move {
                self.reverify_program(params).await;
            });
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
            .inner_join(crate::schema::solana_program_builds::table)
            .filter(program_id.eq(program_address))
            .select(verified_programs::all_columns());

        match signer {
            Some(signer) => query
                .filter(crate::schema::solana_program_builds::signer.eq(signer))
                .first::<VerifiedProgram>(conn)
                .await
                .map_err(|e| {
                    error!("Failed to get verified build: {}", e);
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
                                    .eq(Some(program_authority)))
                                .or(crate::schema::solana_program_builds::signer.is_null()),
                        )
                        .into_boxed();
                }

                filtered_query
                    .first::<VerifiedProgram>(conn)
                    .await
                    .map_err(|e| {
                        error!("Failed to get verified build: {}", e);
                        e.into()
                    })
            }
        }
    }

    pub async fn get_verified_builds_with_signer(
        &self,
        program_address: &str,
    ) -> Result<Vec<(SolanaProgramBuild, Option<VerifiedProgram>)>> {
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
                                sp.signer
                            ORDER BY
                                created_at
                        ) AS rn
                    FROM
                        verified_programs vp
                        LEFT JOIN solana_program_builds sp ON sp.id = vp.solana_build_id
                    WHERE
                        vp.program_id = $1 AND vp.is_verified = true
                ) subquery
            WHERE
                rn = 1
        "#,
        )
        .bind::<diesel::sql_types::Text, _>(program_address)
        .load::<(SolanaProgramBuild, Option<VerifiedProgram>)>(conn)
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
        };

        let program_authority = get_program_authority(&payload.program_id)
            .await
            .unwrap_or(None);

        let params_from_onchain =
            onchain::get_otter_verify_params(&payload.program_id, None, program_authority.clone())
                .await;

        if let Ok((params_from_onchain, _)) = params_from_onchain {
            let _ = self
                .insert_or_update_program_authority(
                    &params_from_onchain.address,
                    program_authority.as_deref(),
                )
                .await;

            let otter_params = SolanaProgramBuildParams::from(params_from_onchain);
            if otter_params != payload {
                info!("Build params from on-chain and database don't match. Re-verifying the build using onchain Metadata.");
                payload = otter_params;
            }
        }

        let build_id = build_params.id;
        let random_file_id = uuid::Uuid::new_v4().to_string();

        tokio::spawn(async move {
            match verification::execute_verification(payload, &build_id, &random_file_id).await {
                Ok(res) => {
                    let _ = self.insert_or_update_verified_build(&res).await;
                    let _ = self
                        .update_build_status(&build_id, JobStatus::Completed)
                        .await;
                }
                Err(err) => {
                    let _ = self.update_build_status(&build_id, JobStatus::Failed).await;
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
}
