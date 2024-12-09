use super::DbClient;
use crate::{db::models::VerifiedProgram, Result};
use diesel_async::RunQueryDsl;
use tracing::{error, info};

/// DbClient helper functions for VerifiedPrograms table to retrieve verified programs
impl DbClient {
    /// Retrieves all verified programs from the database
    /// 
    /// Returns a list of VerifiedProgram structs
    pub async fn get_verified_programs(&self) -> Result<Vec<VerifiedProgram>> {
        use crate::schema::verified_programs::dsl::*;

        let conn = &mut self.get_db_conn().await?;

        info!("Fetching list of verified programs");
        verified_programs
            .load::<VerifiedProgram>(conn)
            .await
            .map_err(|e| {
                error!("Failed to fetch verified programs: {}", e);
                e.into()
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_verified_programs() {
        dotenv::dotenv().ok();
        let db_url = std::env::var("TEST_DATABASE_URL").unwrap();
        let redis_url = std::env::var("TEST_REDIS_URL").unwrap();
        let client = DbClient::new(&db_url, &redis_url);

        let result = client.get_verified_programs().await;
        assert!(result.is_ok());
    }
}
