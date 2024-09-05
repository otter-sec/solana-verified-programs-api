use diesel_async::RunQueryDsl;
use crate::db::models::VerifiedProgram;
use crate::Result;
use super::DbClient;

impl DbClient {
    pub async fn get_verified_programs(&self) -> Result<Vec<VerifiedProgram>> {
        use crate::schema::verified_programs::dsl::*;

        let conn = &mut self.db_pool.get().await?;
        verified_programs.load::<VerifiedProgram>(conn).await.map_err(Into::into)
    }
}
