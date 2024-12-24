use super::DbClient;
use crate::{errors::ApiError, Result};
use diesel::{expression_methods::ExpressionMethods, query_dsl::QueryDsl};
use diesel_async::RunQueryDsl;
use solana_sdk::pubkey::Pubkey;
use tracing::{error, info};

/// DbClient helper functions for ProgramAuthority table
impl DbClient {
    /// Inserts or updates the authority for a Solana program
    pub async fn insert_or_update_program_authority(
        &self,
        program_address: &Pubkey,
        authority_value: Option<&str>,
    ) -> Result<usize> {
        use crate::schema::program_authority::dsl::*;

        let conn = &mut self.get_db_conn().await?;
        let current_time = chrono::Utc::now().naive_utc();
        let program_id_str = program_address.to_string();

        info!(
            "Updating authority for program: {} to: {:?}",
            program_id_str, authority_value
        );

        let result = diesel::insert_into(program_authority)
            .values((
                program_id.eq(&program_id_str),
                authority_id.eq(authority_value.map(|val| val.to_string())),
                last_updated.eq(current_time),
            ))
            .on_conflict(program_id)
            .do_update()
            .set((
                authority_id.eq(authority_value.map(|val| val.to_string())),
                last_updated.eq(current_time),
            ))
            .execute(conn)
            .await
            .map_err(|e| {
                error!("Failed to update authority: {}", e);
                ApiError::Diesel(e)
            })?;

        info!(
            "Successfully updated authority for program: {}",
            program_id_str
        );
        Ok(result)
    }

    /// Retrieves the authority of a program from the database
    pub async fn get_program_authority_from_db(
        &self,
        program_address: &str,
    ) -> Result<Option<String>> {
        use crate::schema::program_authority::dsl::*;

        let conn = &mut self.get_db_conn().await?;

        program_authority
            .select(authority_id)
            .filter(program_id.eq(program_address))
            .first::<Option<String>>(conn)
            .await
            .map_err(|e| {
                error!(
                    "Failed to get authority for program {}: {}",
                    program_address, e
                );
                ApiError::Diesel(e)
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_program_authority() {
        dotenv::dotenv().ok();
        let db_url = std::env::var("TEST_DATABASE_URL").unwrap();
        let redis_url = std::env::var("TEST_REDIS_URL").unwrap();
        let client = DbClient::new(&db_url, &redis_url);

        let program_key = Pubkey::new_unique();
        let authority = "authority123";

        // Test insert
        let insert_result = client
            .insert_or_update_program_authority(&program_key, Some(authority))
            .await;
        assert!(insert_result.is_ok());

        // Test retrieve
        let get_result = client
            .get_program_authority_from_db(&program_key.to_string())
            .await;
        assert!(get_result.is_ok());
        assert_eq!(get_result.unwrap(), Some(authority.to_string()));
    }
}