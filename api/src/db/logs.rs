use super::DbClient;
use crate::db::models::BuildLogs;
use crate::Result;
use diesel::{expression_methods::ExpressionMethods, query_dsl::QueryDsl};
use diesel_async::RunQueryDsl;


impl DbClient {
    pub async fn insert_logs_info(&self, file_id: &str, program_addr: &str) -> Result<usize> {
        use crate::schema::build_logs::dsl::*;
        let conn = &mut self.db_pool.get().await?;
        diesel::insert_into(build_logs)
            .values(BuildLogs {
                id: uuid::Uuid::new_v4().to_string(),
                program_address: program_addr.to_string(),
                file_name: file_id.to_string(),
                created_at: chrono::Utc::now().naive_utc(),
            })
            .execute(conn)
            .await
            .map_err(Into::into)
    }

    pub async fn get_logs_info(&self, program_addr: &str) -> Result<BuildLogs> {
        use crate::schema::build_logs::dsl::*;
        let conn = &mut self.db_pool.get().await?;
        build_logs
            .filter(program_address.eq(program_addr))
            .order(created_at.desc())
            .first::<BuildLogs>(conn)
            .await
            .map_err(Into::into)
    }
}