use super::DbClient;
use crate::db::models::BuildLogs;
use crate::Result;
use diesel::{expression_methods::ExpressionMethods, query_dsl::QueryDsl};
use diesel_async::RunQueryDsl;

impl DbClient {
    pub async fn insert_logs_info(
        &self,
        file_id: &str,
        program_addr: &str,
        build_id: &str,
    ) -> Result<usize> {
        use crate::schema::build_logs::dsl::*;
        let conn = &mut self.db_pool.get().await?;
        diesel::insert_into(build_logs)
            .values(BuildLogs {
                id: build_id.to_string(),
                program_address: program_addr.to_string(),
                file_name: file_id.to_string(),
                created_at: chrono::Utc::now().naive_utc(),
            })
            .execute(conn)
            .await
            .map_err(Into::into)
    }

    pub async fn get_logs_info(&self, build_id: &str) -> Result<BuildLogs> {
        use crate::schema::build_logs::dsl::*;
        let conn = &mut self.db_pool.get().await?;
        build_logs
            .filter(id.eq(build_id))
            .order(created_at.desc())
            .first::<BuildLogs>(conn)
            .await
            .map_err(Into::into)
    }
}
