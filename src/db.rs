use crate::Result;
use diesel::{expression_methods::ExpressionMethods, query_dsl::QueryDsl};
use diesel_async::pooled_connection::AsyncDieselConnectionManager;
use diesel_async::RunQueryDsl;
use diesel_async::{pooled_connection::deadpool::Pool, AsyncPgConnection};
use r2d2_redis::redis::Commands;
use r2d2_redis::{r2d2, RedisConnectionManager};

use crate::builder::get_on_chain_hash;
use crate::models::{
    SolanaProgramBuild, SolanaProgramBuildParams, VerificationResponse, VerifiedProgram,
};

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

    pub async fn insert_or_update_build(&self, payload: &SolanaProgramBuild) -> Result<usize> {
        use crate::schema::solana_program_builds::dsl::*;

        let conn = &mut self.db_pool.get().await?;
        diesel::insert_into(solana_program_builds)
            .values(payload)
            .on_conflict(program_id)
            .do_update()
            .set(payload)
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

    pub async fn get_build_params(&self, program_address: &str) -> Result<SolanaProgramBuild> {
        use crate::schema::solana_program_builds::dsl::*;

        let conn = &mut self.db_pool.get().await?;
        solana_program_builds
            .find(program_address)
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

    // Redis cache SET and Value expiring in 60 seconds
    pub async fn set_cache(&self, program_address: &str, value: &str) -> Result<()> {
        let cache_res = self.redis_pool.get();
        let mut redis_conn = match cache_res {
            Ok(conn) => conn,
            Err(err) => {
                tracing::error!("Redis connection error: {}", err);
                return Err(err.into());
            }
        };
        let _: () = redis_conn.set_ex(program_address, value, 60).unwrap();
        tracing::info!("Cache set for program: {}", program_address);
        Ok(())
    }

    // Redis cache GET program_hash and return the value
    pub async fn get_cache(&self, program_address: &str) -> Result<String> {
        let cache_res = self.redis_pool.get();
        let mut redis_conn = match cache_res {
            Ok(conn) => conn,
            Err(err) => {
                tracing::error!("Redis connection error: {}", err);
                return Err(err.into());
            }
        };
        let res = redis_conn.get(program_address);
        match res {
            Ok(res) => Ok(res),
            Err(err) => {
                tracing::error!("Redis connection error: {}", err);
                Err(err.into())
            }
        }
    }

    pub async fn check_cache(&self, build_hash: &str, program_address: &str) -> Result<bool> {
        // Try to get the program from the cache and check if the hash matches
        let cache_res = self.get_cache(program_address).await;
        match cache_res {
            Ok(res) => {
                if res == build_hash {
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

    /// The function `check_is_program_verified_within_24hrs` checks if a program is verified within the last 24 hours
    /// and rebuilds and verifies it if it is not.
    ///
    /// Arguments:
    ///
    /// * `program_address`: The `program_address` parameter is a string that represents the address of a
    /// program. It is used to query the database and check if the program is verified.
    ///
    /// Returns: Whether the program is verified or not.
    pub async fn check_is_program_verified_within_24hrs(
        self,
        program_address: String,
    ) -> Result<VerificationResponse> {
        let res = self.get_verified_build(&program_address).await;
        match res {
            Ok(res) => {
                let cache_result = self
                    .check_cache(&res.executable_hash, &program_address)
                    .await;

                if let Ok(found) = cache_result {
                    if found {
                        tracing::info!("Cache hit for program: {}", program_address);
                        return Ok({
                            VerificationResponse {
                                is_verified: true,
                                on_chain_hash: res.on_chain_hash,
                                executable_hash: res.executable_hash,
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
                        tracing::info!("On chain hash doesn't match. Rebuilding the program.");
                        self.update_onchain_hash(&program_address, &on_chain_hash)
                            .await?;
                    }
                    Ok({
                        VerificationResponse {
                            is_verified: on_chain_hash == res.on_chain_hash,
                            on_chain_hash,
                            executable_hash: res.executable_hash,
                        }
                    })
                } else {
                    tracing::info!("Failed to get On chain hash. Returning the cached value.");
                    Ok({
                        VerificationResponse {
                            is_verified: res.on_chain_hash == res.executable_hash,
                            on_chain_hash: res.on_chain_hash,
                            executable_hash: res.executable_hash,
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
                        }
                    });
                }
                Err(err)
            }
        }
    }

    pub async fn check_is_build_params_exists_already(
        &self,
        payload: &SolanaProgramBuildParams,
    ) -> Result<(bool, Option<VerificationResponse>)> {
        let build = self.get_build_params(&payload.program_id).await?;
        tracing::info!("DB {:?}", build);
        tracing::info!("Payload {:?}", payload);

        let res = build.repository == payload.repository
            && build.commit_hash == payload.commit_hash
            && build.lib_name == payload.lib_name
            && build.bpf_flag == payload.bpf_flag.unwrap_or(false)
            && build.base_docker_image == payload.base_image
            && build.mount_path == payload.mount_path
            && build.cargo_args
                == if payload.cargo_args.is_none() {
                    Some([].to_vec())
                } else {
                    payload.cargo_args.clone()
                };
        if res {
            tracing::info!(
                "Build params already exists for this program :{}",
                payload.program_id
            );
            let verification_status = self.get_verified_build(&payload.program_id).await;
            match verification_status {
                Ok(verification_status) => {
                    return Ok((
                        true,
                        Some(VerificationResponse {
                            is_verified: verification_status.is_verified,
                            on_chain_hash: verification_status.on_chain_hash,
                            executable_hash: verification_status.executable_hash,
                        }),
                    ))
                }
                Err(_) => {
                    return Ok((true, None));
                }
            }
        }
        Ok((res, None))
    }
}
