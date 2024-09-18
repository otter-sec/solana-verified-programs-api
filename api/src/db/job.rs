use super::DbClient;
use crate::db::models::SolanaProgramBuild;
use crate::Result;
use diesel::{expression_methods::ExpressionMethods, query_dsl::QueryDsl};
use diesel_async::RunQueryDsl;

impl DbClient {
    pub async fn get_job(&self, uid: &str) -> Result<SolanaProgramBuild> {
        use crate::schema::solana_program_builds::dsl::*;

        let conn = &mut self.db_pool.get().await?;
        solana_program_builds
            .filter(id.eq(uid))
            .first::<SolanaProgramBuild>(conn)
            .await
            .map_err(Into::into)
    }

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
}
