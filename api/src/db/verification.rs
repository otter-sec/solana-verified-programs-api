use crate::builder::{self, get_on_chain_hash};
use crate::db::models::{VerificationResponse, SolanaProgramBuild, SolanaProgramBuildParams, JobStatus, VerifiedProgram};
use crate::Result;
use super::DbClient;
use diesel::{expression_methods::ExpressionMethods, query_dsl::QueryDsl};
use diesel_async::RunQueryDsl;

impl DbClient {
    pub async fn check_is_verified(self, program_address: String) -> Result<VerificationResponse> {
        let res = self.get_verified_build(&program_address).await;
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
                            repo_url: builder::get_repo_url(&build_params),
                            last_verified_at: Some(res.verified_at),
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
                        self.reverify_program(build_params.clone());
                    }
                    Ok(VerificationResponse {
                        is_verified: on_chain_hash == res.executable_hash,
                        on_chain_hash,
                        executable_hash: res.executable_hash,
                        repo_url: builder::get_repo_url(&build_params),
                        last_verified_at: Some(res.verified_at),
                    })
                } else {
                    tracing::info!("Failed to get On chain hash. Returning the cached value.");
                    Ok(VerificationResponse {
                        is_verified: res.on_chain_hash == res.executable_hash,
                        on_chain_hash: res.on_chain_hash,
                        executable_hash: res.executable_hash,
                        repo_url: builder::get_repo_url(&build_params),
                        last_verified_at: Some(res.verified_at),
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
                    });
                }
                Err(err)
            }
        }
    }

    pub async fn get_verified_build(&self, program_address: &str) -> Result<VerifiedProgram> {
        use crate::schema::verified_programs::dsl::*;

        let conn = &mut self.db_pool.get().await?;
        verified_programs
            .filter(crate::schema::verified_programs::program_id.eq(program_address))
            .first::<VerifiedProgram>(conn)
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
            .on_conflict(program_id)
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

    pub fn reverify_program(self, build_params: SolanaProgramBuild) {
        let payload = SolanaProgramBuildParams {
            program_id: build_params.program_id,
            repository: build_params.repository,
            commit_hash: build_params.commit_hash,
            lib_name: build_params.lib_name,
            base_image: build_params.base_docker_image,
            mount_path: build_params.mount_path,
            bpf_flag: Some(build_params.bpf_flag),
            cargo_args: build_params.cargo_args,
        };

        let build_id = build_params.id;

        //run task in background
        tokio::spawn(async move {
            match builder::verify_build(payload, &build_id).await {
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
}