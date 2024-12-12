use chrono::Utc;
use diesel::{expression_methods::ExpressionMethods, query_dsl::QueryDsl};
use diesel_async::RunQueryDsl;

use super::DbClient;
use crate::{db::models::BuildLogs, Result};

/// DbClient helper functions for BuildLogs table to insert and retrieve build logs information
impl DbClient {
    /// Stores build log information in the database
    pub async fn insert_logs_info(
        &self,
        file_id: &str,
        program_addr: &str,
        build_id: &str,
    ) -> Result<usize> {
        use crate::schema::build_logs::dsl::*;

        let conn = &mut self.get_db_conn().await?;
        diesel::insert_into(build_logs)
            .values(BuildLogs {
                id: build_id.to_string(),
                program_address: program_addr.to_string(),
                file_name: file_id.to_string(),
                created_at: Utc::now().naive_utc(),
            })
            .execute(conn)
            .await
            .map_err(Into::into)
    }

    /// Retrieves build log information by build ID
    pub async fn get_logs_info(&self, build_id: &str) -> Result<BuildLogs> {
        use crate::schema::build_logs::dsl::*;

        let conn = &mut self.get_db_conn().await?;
        build_logs
            .filter(id.eq(build_id))
            .order(created_at.desc())
            .first::<BuildLogs>(conn)
            .await
            .map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_logs_crud() {
        dotenv::dotenv().ok();
        let db_url = std::env::var("TEST_DATABASE_URL").unwrap();
        let redis_url = std::env::var("TEST_REDIS_URL").unwrap();
        let client = DbClient::new(&db_url, &redis_url);

        let file_id = "test_file";
        let program_addr = "test_program";
        let build_id = "test_build";

        // Test insert
        let insert_result = client
            .insert_logs_info(file_id, program_addr, build_id)
            .await;
        assert!(insert_result.is_ok());

        // Test retrieve
        let get_result = client.get_logs_info(build_id).await;
        assert!(get_result.is_ok());
    }
}
