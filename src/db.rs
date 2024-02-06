use diesel::{expression_methods::ExpressionMethods, query_dsl::QueryDsl};
use diesel_async::pooled_connection::AsyncDieselConnectionManager;
use diesel_async::RunQueryDsl;
use diesel_async::{pooled_connection::deadpool::Pool, AsyncPgConnection};
use r2d2_redis::redis::{Commands, FromRedisValue, Value};
use r2d2_redis::{r2d2, RedisConnectionManager};

use crate::builder::{self, get_on_chain_hash};
use crate::errors::ApiError;
use crate::models::{
    JobStatus, SolanaProgramBuild, SolanaProgramBuildParams, VerificationResponse, VerifiedProgram,
};
use crate::Result;

#[derive(Clone)]
pub struct DbClient {
    pub db_pool: Pool<AsyncPgConnection>,
    pub redis_pool: r2d2::Pool<RedisConnectionManager>,
}

impl DbClient {
    pub fn new(db_url: &str, redis_url: &str) -> Self {
        let config = AsyncDieselConnectionManager::<diesel_async::AsyncPgConnection>::new(db_url);
        let postgres_pool = Pool::builder(config)
            .build()
            .expect("Failed to create DB Pool");
        let manager = RedisConnectionManager::new(redis_url).expect(
            "Failed to create Redis connection manager. Check that REDIS_URL is set in .env file",
        );
        let redis_pool = r2d2::Pool::builder().build(manager).expect(
            "Failed to create Redis connection pool. Check that REDIS_URL is set in .env file",
        );

        Self {
            db_pool: postgres_pool,
            redis_pool,
        }
    }

    pub async fn insert_build_params(&self, payload: &SolanaProgramBuild) -> Result<usize> {
        use crate::schema::solana_program_builds::dsl::*;

        let conn = &mut self.db_pool.get().await?;
        diesel::insert_into(solana_program_builds)
            .values(payload)
            .execute(conn)
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

    pub async fn check_for_dupliate(
        &self,
        payload: &SolanaProgramBuildParams,
    ) -> Result<SolanaProgramBuild> {
        use crate::schema::solana_program_builds::dsl::*;

        let conn = &mut self.db_pool.get().await?;

        let mut query = solana_program_builds.into_boxed();

        query = query.filter(program_id.eq(payload.program_id.to_owned()));
        query = query.filter(repository.eq(payload.repository.to_owned()));

        // commit_hash is optional
        if let Some(hash) = &payload.commit_hash {
            query = query.filter(commit_hash.eq(hash));
        }

        // lib_name is optional
        if let Some(lib) = &payload.lib_name {
            query = query.filter(lib_name.eq(lib));
        }

        // bpf_flag is optional
        if let Some(bpf) = &payload.bpf_flag {
            query = query.filter(bpf_flag.eq(bpf));
        }

        // base_docker_image is optional
        if let Some(base) = &payload.base_image {
            query = query.filter(base_docker_image.eq(base));
        }

        // mount_path is optional
        if let Some(mount) = &payload.mount_path {
            query = query.filter(mount_path.eq(mount));
        }

        // cargo_args is optional
        if let Some(args) = payload.cargo_args.clone() {
            query = query.filter(cargo_args.eq(args));
        }

        query
            .first::<SolanaProgramBuild>(conn)
            .await
            .map_err(Into::into)
    }

    pub async fn get_build_params(&self, program_address: &str) -> Result<SolanaProgramBuild> {
        use crate::schema::solana_program_builds::dsl::*;

        let conn = &mut self.db_pool.get().await?;
        solana_program_builds
            .filter(crate::schema::solana_program_builds::program_id.eq(program_address))
            .first::<SolanaProgramBuild>(conn)
            .await
            .map_err(Into::into)
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

    // Redis cache SET and Value expiring in 60 seconds
    pub async fn set_cache(&self, program_address: &str, value: &str) -> Result<()> {
        let cache_res = self.redis_pool.get();
        let mut redis_conn = match cache_res {
            Ok(conn) => conn,
            Err(err) => {
                tracing::error!("Redis connection error: {}", err);
                return Err(ApiError::from(err));
            }
        };
        redis_conn
            .set_ex(program_address, value, 60)
            .map_err(|err| {
                tracing::error!("Redis SET failed: {}", err);
                ApiError::from(err)
            })?;
        tracing::info!("Cache set for program: {}", program_address);
        Ok(())
    }

    // Redis cache GET program_hash and return the value
    pub async fn get_cache(&self, program_address: &str) -> Result<String> {
        let cache_res = self.redis_pool.get().map_err(|err| {
            tracing::error!("Redis connection error: {}", err);
            ApiError::from(err)
        })?;

        let mut redis_conn = cache_res;

        let value: Value = redis_conn.get(program_address).map_err(|err| {
            tracing::error!("Redis connection error: {}", err);
            ApiError::from(err)
        })?;

        match value {
            Value::Nil => Err(ApiError::Custom(format!(
                "Record not found for program: {}",
                program_address
            ))),
            _ => FromRedisValue::from_redis_value(&value).map_err(|err| {
                tracing::error!("Redis Value error: {}", err);
                ApiError::from(err)
            }),
        }
    }

    pub async fn check_cache(&self, hash: &str, program_address: &str) -> Result<bool> {
        // Try to get the program from the cache and check if the hash matches
        let cache_res = self.get_cache(program_address).await;
        match cache_res {
            Ok(res) => {
                if res == hash {
                    tracing::info!(
                        "Cache hit for program: {} And hash matches",
                        program_address
                    );
                    Ok(true)
                } else {
                    tracing::info!(
                        "Cache hit for program: {} And hash doesn't matches",
                        program_address
                    );
                    Ok(false)
                }
            }
            Err(err) => {
                tracing::error!("Redis connection error: {}", err);
                Ok(false)
            }
        }
    }

    /// The function `check_is_verified` checks if a program is verified or not.
    /// It first checks onchain hash from chache and build hash from the database and compares them.
    /// If they match, it returns true. If they don't match, it updates the onchain hash
    /// in the database and starts a new build if onchain hash in db is also different from what we got
    ///
    /// Arguments:
    ///
    /// * `program_address`: The `program_address` parameter is a string that represents the address of a
    /// program. It is used to query the database and check if the program is verified.
    ///
    /// Returns: Whether the program is verified or not.
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
                        tracing::info!("Cache mached for program: {}", program_address);
                        return Ok({
                            VerificationResponse {
                                is_verified: true,
                                on_chain_hash: res.on_chain_hash,
                                executable_hash: res.executable_hash,
                                repo_url: builder::get_repo_url(&build_params),
                                last_verified_at: Some(res.verified_at),
                            }
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
                    Ok({
                        VerificationResponse {
                            is_verified: on_chain_hash == res.executable_hash,
                            on_chain_hash,
                            executable_hash: res.executable_hash,
                            repo_url: builder::get_repo_url(&build_params),
                            last_verified_at: Some(res.verified_at),
                        }
                    })
                } else {
                    tracing::info!("Failed to get On chain hash. Returning the cached value.");
                    Ok({
                        VerificationResponse {
                            is_verified: res.on_chain_hash == res.executable_hash,
                            on_chain_hash: res.on_chain_hash,
                            executable_hash: res.executable_hash,
                            repo_url: builder::get_repo_url(&build_params),
                            last_verified_at: Some(res.verified_at),
                        }
                    })
                }
            }
            Err(err) => {
                if err.to_string() == "Record not found" {
                    tracing::info!("{}: Program record not found in database", program_address);
                    return Ok({
                        VerificationResponse {
                            is_verified: false,
                            on_chain_hash: "".to_string(),
                            executable_hash: "".to_string(),
                            repo_url: "".to_string(),
                            last_verified_at: None,
                        }
                    });
                }
                Err(err)
            }
        }
    }

    // Get solana_program_builds status by id
    pub async fn get_job(&self, uid: &str) -> Result<SolanaProgramBuild> {
        use crate::schema::solana_program_builds::dsl::*;

        let conn = &mut self.db_pool.get().await?;
        solana_program_builds
            .filter(id.eq(uid))
            .first::<SolanaProgramBuild>(conn)
            .await
            .map_err(Into::into)
    }

    // Update solana_program_builds by id and set status
    pub async fn update_build_status(&self, uid: &str, job_status: String) -> Result<usize> {
        use crate::schema::solana_program_builds::dsl::*;
        let conn = &mut self.db_pool.get().await?;
        diesel::update(solana_program_builds)
            .filter(id.eq(uid))
            .set(crate::schema::solana_program_builds::status.eq(job_status))
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
