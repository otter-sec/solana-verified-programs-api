use diesel::{expression_methods::ExpressionMethods, query_dsl::QueryDsl};
use diesel_async::RunQueryDsl;

use super::DbClient;
use crate::{
    db::models::{JobStatus, SolanaProgramBuild},
    Result,
};

/// DbClient helper functions for SolanaProgramBuilds table to update job status and retrieve job
impl DbClient {
    /// Retrieves a job by its unique identifier
    pub async fn get_job(&self, uid: &str) -> Result<SolanaProgramBuild> {
        use crate::schema::solana_program_builds::dsl::*;

        let conn = &mut self.get_db_conn().await?;
        solana_program_builds
            .filter(id.eq(uid))
            .first::<SolanaProgramBuild>(conn)
            .await
            .map_err(Into::into)
    }

    /// Updates the status of a build job
    pub async fn update_build_status(&self, uid: &str, new_status: JobStatus) -> Result<usize> {
        use crate::schema::solana_program_builds::dsl::*;

        let conn = &mut self.get_db_conn().await?;
        diesel::update(solana_program_builds)
            .filter(id.eq(uid))
            .set(status.eq(String::from(new_status)))
            .execute(conn)
            .await
            .map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::models::JobStatus;

    #[tokio::test]
    async fn test_job_status_update() {
        dotenv::dotenv().ok();
        let db_url = std::env::var("TEST_DATABASE_URL").unwrap();
        let redis_url = std::env::var("TEST_REDIS_URL").unwrap();
        let client = DbClient::new(&db_url, &redis_url);

        let result = client
            .update_build_status("test_id", JobStatus::Completed)
            .await;

        assert!(result.is_ok());
    }
}
