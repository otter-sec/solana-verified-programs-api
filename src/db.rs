use crate::Result;
use diesel::{expression_methods::ExpressionMethods, query_dsl::QueryDsl};
use diesel_async::pooled_connection::AsyncDieselConnectionManager;
use diesel_async::RunQueryDsl;
use diesel_async::{pooled_connection::deadpool::Pool, AsyncPgConnection};

use crate::builder::reverify;
use crate::models::{SolanaProgramBuild, SolanaProgramBuildParams, VerifiedProgram};

#[derive(Clone)]
pub struct DbClient {
    pub db_pool: Pool<AsyncPgConnection>,
}

impl DbClient {
    pub fn new(db_url: &str) -> Self {
        let config = AsyncDieselConnectionManager::<diesel_async::AsyncPgConnection>::new(db_url);
        let pool = Pool::builder(config)
            .build()
            .expect("Failed to create DB Pool");
        Self { db_pool: pool }
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

    pub async fn update_verified_build_time(&self, program_address: &str) -> Result<usize> {
        use crate::schema::verified_programs::dsl::*;

        let time_now = chrono::Utc::now().naive_utc();
        let conn = &mut self.db_pool.get().await?;
        diesel::update(verified_programs)
            .filter(program_id.eq(program_address))
            .set(verified_at.eq(time_now))
            .execute(conn)
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
    ) -> Result<bool> {
        let res = self.get_verified_build(&program_address).await;
        match res {
            Ok(res) => {
                // check if the program is verified less than 24 hours ago
                let now = chrono::Utc::now().naive_utc();
                let verified_at = res.verified_at;
                let diff = now - verified_at;
                if diff.num_hours() >= 24 && res.is_verified {
                    // if the program is verified more than 24 hours ago, rebuild and verify
                    let payload_last_build = self.get_build_params(&program_address).await?;
                    tokio::spawn(async move {
                        let status = reverify(payload_last_build, res.on_chain_hash).await;
                        match status {
                            Ok(true) => {
                                let _ = self.update_verified_build_time(&program_address).await;
                                tracing::info!("Re-verification not needed")
                            }
                            Ok(false) => tracing::error!("Re-verification needed"),
                            Err(_) => tracing::error!("Re-Verify failed"),
                        }
                    });
                }
                Ok(res.is_verified)
            }
            Err(err) => {
                if err.to_string() == "Record not found" {
                    tracing::info!("{}: Program record not found in database", program_address);
                    return Ok(false);
                }
                Err(err)
            }
        }
    }

    pub async fn check_is_build_params_exists_already(
        &self,
        payload: &SolanaProgramBuildParams,
    ) -> Result<bool> {
        let build = self.get_build_params(&payload.program_id).await?;
        let res = build.repository == payload.repository
            && build.commit_hash == payload.commit_hash
            && build.lib_name == payload.lib_name
            && build.bpf_flag == payload.bpf_flag.unwrap_or(false);
        if res {
            tracing::info!(
                "Build params already exists for this program {}",
                payload.program_id
            );
        }
        Ok(res)
    }
}
