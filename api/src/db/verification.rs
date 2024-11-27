use super::models::VerificationResponseWithSigner;
use super::DbClient;
use crate::db::models::{
    JobStatus, SolanaProgramBuild, SolanaProgramBuildParams, VerificationResponse, VerifiedProgram,
    DEFAULT_SIGNER,
};
use crate::services::{get_on_chain_hash, get_repo_url, onchain, verification};
use crate::Result;
use diesel::expression_methods::BoolExpressionMethods;
use diesel::{expression_methods::ExpressionMethods, query_dsl::QueryDsl};
use diesel::{sql_query, Table};
use diesel_async::RunQueryDsl;

impl DbClient {
    pub async fn check_is_verified(
        self,
        program_address: String,
        signer: Option<String>,
    ) -> Result<VerificationResponse> {
        let res = self.get_verified_build(&program_address, signer).await;
        match res {
            Ok(res) => {
                let cache_result = self
                    .check_cache(&res.executable_hash, &program_address)
                    .await;

                let build_params = self.get_build_params(&program_address).await?;

                if let Ok(matched) = cache_result {
                    if matched {
                        tracing::info!("Cache matched for program: {}", program_address);
                        return Ok(VerificationResponse {
                            is_verified: true,
                            on_chain_hash: res.on_chain_hash,
                            executable_hash: res.executable_hash,
                            repo_url: get_repo_url(&build_params),
                            last_verified_at: Some(res.verified_at),
                            commit: build_params.commit_hash.unwrap_or_default(),
                        });
                    }
                }

                let on_chain_hash = get_on_chain_hash(&program_address).await;

                if let Ok(on_chain_hash) = on_chain_hash {
                    self.set_cache(&program_address, &on_chain_hash).await?;
                    if on_chain_hash == res.on_chain_hash {
                        tracing::info!("On chain hash matches. Returning the cached value.");
                    } else {
                        tracing::info!("On chain hash doesn't match.");
                        self.update_onchain_hash(
                            &program_address,
                            &on_chain_hash,
                            on_chain_hash == res.executable_hash,
                        )
                        .await?;
                        // run re-verification task in background
                        let params_cloned = build_params.clone();
                        tokio::spawn(async move {
                            self.reverify_program(params_cloned).await;
                        });
                    }
                    Ok(VerificationResponse {
                        is_verified: on_chain_hash == res.executable_hash,
                        on_chain_hash,
                        executable_hash: res.executable_hash,
                        repo_url: get_repo_url(&build_params),
                        last_verified_at: Some(res.verified_at),
                        commit: build_params.commit_hash.unwrap_or_default(),
                    })
                } else {
                    tracing::info!("Failed to get On chain hash. Returning the cached value.");
                    Ok(VerificationResponse {
                        is_verified: res.on_chain_hash == res.executable_hash,
                        on_chain_hash: res.on_chain_hash,
                        executable_hash: res.executable_hash,
                        repo_url: get_repo_url(&build_params),
                        last_verified_at: Some(res.verified_at),
                        commit: build_params.commit_hash.unwrap_or_default(),
                    })
                }
            }
            Err(err) => {
                if err.to_string() == "Record not found" {
                    tracing::info!("{}: Program record not found in database", program_address);
                    return Ok(VerificationResponse {
                        is_verified: false,
                        on_chain_hash: "".to_string(),
                        executable_hash: "".to_string(),
                        repo_url: "".to_string(),
                        last_verified_at: None,
                        commit: "".to_string(),
                    });
                }
                Err(err)
            }
        }
    }

    pub async fn get_all_verification_info(
        self,
        program_address: String,
    ) -> Result<Vec<VerificationResponseWithSigner>> {
        let res = self.get_verified_builds_with_signer(&program_address).await;
        match res {
            Ok(res) => {
                let hash = if let Ok(cache_result) = self.get_cache(&program_address).await {
                    Some(cache_result)
                } else {
                    let on_chain_hash = get_on_chain_hash(&program_address).await;
                    if let Ok(on_chain_hash) = on_chain_hash {
                        self.set_cache(&program_address, &on_chain_hash).await?;
                        Some(on_chain_hash)
                    } else {
                        None
                    }
                };

                let mut is_verification_needed = false;

                let mut verification_responses = vec![];
                for (build, verified_build) in res {
                    if verified_build.is_none() {
                        tracing::info!("Verified build not found for {:?}", build.signer);
                        continue;
                    }
                    let verified_build = verified_build.unwrap();
                    let is_verified = if let Some(hash) = hash.clone() {
                        // Check if the on-chain hash matches the cached build hash
                        if *hash != verified_build.executable_hash {
                            tracing::info!("On chain hash doesn't match.");
                            // Update the on-chain hash in the database
                            self.update_onchain_hash(
                                &program_address,
                                &hash,
                                hash == verified_build.executable_hash,
                            )
                            .await
                            .unwrap();
                            is_verification_needed = true;
                        } else {
                            tracing::info!("On chain hash matches. Returning the cached value.");
                        }
                        *hash == verified_build.on_chain_hash
                    } else {
                        verified_build.executable_hash == verified_build.on_chain_hash
                    };

                    verification_responses.push(VerificationResponseWithSigner {
                        verification_response: VerificationResponse {
                            is_verified,
                            on_chain_hash: verified_build.on_chain_hash,
                            executable_hash: verified_build.executable_hash,
                            repo_url: get_repo_url(&build),
                            last_verified_at: Some(verified_build.verified_at),
                            commit: build.commit_hash.unwrap_or_default(),
                        },
                        signer: build.signer.unwrap_or(DEFAULT_SIGNER.to_string()),
                    })
                }

                // Run re-verification task in background if needed
                if is_verification_needed {
                    let params = self.get_build_params(&program_address).await?;
                    tokio::spawn(async move {
                        self.reverify_program(params).await;
                    });
                }
                Ok(verification_responses)
            }
            Err(err) => {
                tracing::error!("Error getting data from database: {}", err);
                if err.to_string() == "Record not found" {
                    tracing::info!("{}: Program record not found in database", program_address);
                    return Ok(vec![]);
                }
                Err(err)
            }
        }
    }

    pub async fn get_verified_build(
        &self,
        program_address: &str,
        signer: Option<String>,
    ) -> Result<VerifiedProgram> {
        use crate::schema::verified_programs::dsl::*;

        tracing::info!("Getting verified build for {:?}", program_address);

        let conn = &mut self.db_pool.get().await?;
        let query = verified_programs
            .inner_join(crate::schema::solana_program_builds::table)
            .filter(crate::schema::verified_programs::program_id.eq(program_address))
            .select(verified_programs::all_columns());

        match signer {
            Some(signer) => query
                .filter(crate::schema::solana_program_builds::signer.eq(signer))
                .first::<VerifiedProgram>(conn)
                .await
                .map_err(Into::into),
            None => query
                .filter(
                    crate::schema::solana_program_builds::signer
                        .eq(DEFAULT_SIGNER.to_string())
                        .or(crate::schema::solana_program_builds::signer.is_null()),
                )
                .first::<VerifiedProgram>(conn)
                .await
                .map_err(Into::into),
        }
    }

    pub async fn get_verified_builds_with_signer(
        &self,
        program_address: &str,
    ) -> Result<Vec<(SolanaProgramBuild, Option<VerifiedProgram>)>> {
        let conn = &mut self.db_pool.get().await?;
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
                        vp.program_id = $1
                ) subquery
            WHERE
                rn = 1
        "#,
        )
        .bind::<diesel::sql_types::Text, _>(program_address)
        .load::<(SolanaProgramBuild, Option<VerifiedProgram>)>(conn)
        .await
        .map_err(Into::into)
    }

    pub async fn insert_or_update_verified_build(
        &self,
        payload: &VerifiedProgram,
    ) -> Result<usize> {
        use crate::schema::verified_programs::dsl::*;

        let conn = &mut self.db_pool.get().await?;
        diesel::insert_into(verified_programs)
            .values(payload)
            // Should never get hit
            .on_conflict(id)
            .do_update()
            .set(payload)
            .execute(conn)
            .await
            .map_err(Into::into)
    }

    pub async fn update_onchain_hash(
        &self,
        program_address: &str,
        on_chainhash: &str,
        isverified: bool,
    ) -> Result<usize> {
        use crate::schema::verified_programs::dsl::*;
        let conn = &mut self.db_pool.get().await?;
        diesel::update(verified_programs)
            .filter(program_id.eq(program_address))
            .set((
                crate::schema::verified_programs::on_chain_hash.eq(on_chainhash),
                crate::schema::verified_programs::is_verified.eq(isverified),
                crate::schema::verified_programs::verified_at.eq(chrono::Utc::now().naive_utc()),
            ))
            .execute(conn)
            .await
            .map_err(Into::into)
    }

    pub async fn reverify_program(self, build_params: SolanaProgramBuild) {
        tracing::info!("Re-verifying the build.");
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

        // Get the build params from the on-chain Metadata
        let params_from_onchain = onchain::get_otter_verify_params(&payload.program_id, None).await;

        if let Ok(params_from_onchain) = params_from_onchain {
            // Compare the build params from on-chain and the build params from the database
            let otter_params = SolanaProgramBuildParams::from(params_from_onchain);
            if otter_params != payload {
                tracing::info!("Build params from on-chain and database don't match. Re-verifying the build using onchain Metadata.");
                payload = otter_params;
            } else {
                tracing::info!(
                    "Build params from on-chain and database match. Re-verifying the build"
                );
            }
        } else if let Err(err) = params_from_onchain {
            tracing::error!("Error getting on-chain params: {:?}", err);
            tracing::error!("Re-verifying the build using the build params from the database.");
        }

        // id of the build from the database
        let build_id = build_params.id;

        //run task in background
        tokio::spawn(async move {
            let random_file_id = uuid::Uuid::new_v4().to_string();
            match verification::verify_build(payload, &build_id, &random_file_id).await {
                Ok(res) => {
                    let _ = self.insert_or_update_verified_build(&res).await;
                    let _ = self
                        .update_build_status(&build_id, JobStatus::Completed.into())
                        .await;
                }
                Err(err) => {
                    let _ = self
                        .update_build_status(&build_id, JobStatus::Failed.into())
                        .await;
                    tracing::error!("Error verifying build: {:?}", err);
                    tracing::error!(
                        "We encountered an unexpected error during the verification process."
                    );
                }
            }
        });
    }

    pub async fn unverify_program(
        &self,
        program_address: &str,
        on_chainhash: &str,
    ) -> Result<usize> {
        use crate::schema::verified_programs::dsl::*;
        let conn = &mut self.db_pool.get().await?;
        diesel::update(verified_programs)
            .filter(program_id.eq(program_address))
            .set((
                crate::schema::verified_programs::on_chain_hash.eq(on_chainhash),
                crate::schema::verified_programs::is_verified.eq(false),
                crate::schema::verified_programs::verified_at.eq(chrono::Utc::now().naive_utc()),
            ))
            .execute(conn)
            .await
            .map_err(Into::into)
    }
}
