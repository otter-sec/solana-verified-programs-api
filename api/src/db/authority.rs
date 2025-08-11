use super::DbClient;
use crate::{
    db::{models::ProgramAuthorityData, redis::PROGRAM_AUTHORITY_CACHE_EXPIRY_SECONDS},
    errors::ApiError,
    Result,
};
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
        program_is_frozen: bool,
        program_is_closed: Option<bool>,
    ) -> Result<usize> {
        use crate::schema::program_authority::dsl::*;

        let conn = &mut self.get_db_conn().await?;
        let current_time = chrono::Utc::now().naive_utc();
        let program_id_str = program_address.to_string();

        // Fetch the saved record from the database
        let saved_record = program_authority
            .select((authority_id, is_frozen, is_closed))
            .filter(program_id.eq(&program_id_str))
            .first::<(Option<String>, bool, bool)>(conn)
            .await;

        match saved_record {
            Ok((existing_authority, existing_is_frozen, existing_is_closed)) => {
                info!(
                    "Program authority found for program_id {}: {:?}, is_frozen: {}, is_closed: {}",
                    program_id_str, existing_authority, existing_is_frozen, existing_is_closed
                );


                if existing_authority.as_deref() == authority_value
                    && existing_is_frozen == program_is_frozen
                    && existing_is_closed == program_is_closed.unwrap_or(false)
                {
                    info!(
                        "Authority for program_id {} is already the same. Skipping update.",
                        program_id_str
                    );
                    return Ok(0); // Return 0 to indicate no update was performed
                }
            }
            Err(diesel::result::Error::NotFound) => {
                info!(
                    "No existing program authority found for program_id {}. Proceeding to insert.",
                    program_id_str
                );
            }
            Err(e) => {
                info!(
                    "Failed to fetch authority for program_id {}: {}",
                    program_id_str, e
                );
            }
        }

        // Insert or update the record
        info!(
            "Updating authority for program: {} to: {:?}",
            program_id_str, authority_value
        );

        let result = diesel::insert_into(program_authority)
            .values((
                program_id.eq(&program_id_str),
                authority_id.eq(authority_value.map(|val| val.to_string())),
                is_frozen.eq(program_is_frozen),
                is_closed.eq(program_is_closed.unwrap_or(false)),
                last_updated.eq(current_time),
            ))
            .on_conflict(program_id)
            .do_update()
            .set((
                authority_id.eq(authority_value.map(|val| val.to_string())),
                is_frozen.eq(program_is_frozen),
                is_closed.eq(program_is_closed.unwrap_or(false)),
                last_updated.eq(current_time),
            ))
            .execute(conn)
            .await
            .map_err(|e| {
                error!("Failed to update authority: {}", e);
                ApiError::Diesel(e)
            })?;

        // Invalidate cache after successful update
        let cache_key = format!("program_authority:{program_id_str}");
        let _ = self
            .set_cache_with_expiry(
                &cache_key,
                authority_value.unwrap_or("NULL"),
                PROGRAM_AUTHORITY_CACHE_EXPIRY_SECONDS,
            )
            .await;

        info!(
            "Successfully updated authority for program: {}",
            program_id_str
        );
        Ok(result)
    }

    /// Retrieves the authority of a program from the database with caching
    pub async fn get_program_authority_from_db(
        &self,
        program_address: &str,
    ) -> Result<Option<String>> {
        let cache_key = format!("program_authority:{program_address}");

        // Try to get from cache first
        if let Ok(cached_value) = self.get_cache(&cache_key).await {
            if cached_value == "NULL" {
                return Ok(None);
            }
            return Ok(Some(cached_value));
        }

        use crate::schema::program_authority::dsl::*;
        let conn = &mut self.get_db_conn().await?;

        let result = program_authority
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
            })?;

        // Cache the result with longer expiry for program authorities
        match &result {
            Some(authority) => {
                let _ = self
                    .set_cache_with_expiry(
                        &cache_key,
                        authority,
                        crate::db::redis::PROGRAM_AUTHORITY_CACHE_EXPIRY_SECONDS,
                    )
                    .await;
            }
            None => {
                let _ = self
                    .set_cache_with_expiry(
                        &cache_key,
                        "NULL",
                        crate::db::redis::PROGRAM_AUTHORITY_CACHE_EXPIRY_SECONDS,
                    )
                    .await;
            }
        }

        Ok(result)
    }
    /// Retrieves complete program authority data (authority, frozen, closed) in a single query
    /// Returns None if no record is found
    pub async fn get_program_authority_data(
        &self,
        program_address: &str,
    ) -> Result<Option<ProgramAuthorityData>> {
        use crate::schema::program_authority::dsl::*;

        let conn = &mut self.get_db_conn().await?;

        match program_authority
            .select((authority_id, is_frozen, is_closed))
            .filter(program_id.eq(program_address))
            .first::<(Option<String>, bool, bool)>(conn)
            .await
        {
            Ok((auth, frozen, closed)) => Ok(Some(ProgramAuthorityData {
                authority: auth,
                is_frozen: frozen,
                is_closed: closed,
            })),
            Err(diesel::result::Error::NotFound) => Ok(None),
            Err(e) => {
                error!(
                    "Failed to get program authority data for {}: {}",
                    program_address, e
                );
                Err(ApiError::Diesel(e))
            }
        }
    }

    /// Checks if a program is frozen in the database.
    /// Returns `false` if no record is found.
    pub async fn is_program_frozen(&self, program_address: &str) -> Result<bool> {
        use crate::schema::program_authority::dsl::*;

        let conn = &mut self.get_db_conn().await?;

        match program_authority
            .select(is_frozen)
            .filter(program_id.eq(program_address))
            .first::<bool>(conn)
            .await
        {
            Ok(frozen) => Ok(frozen),
            Err(diesel::result::Error::NotFound) => Ok(false),
            Err(e) => {
                error!("Failed to check if program is frozen: {}", e);
                Err(ApiError::Diesel(e))
            }
        }
    }

    /// Checks if a program is closed in the database.
    /// Returns `false` if no record is found.
    pub async fn is_program_closed(&self, program_address: &str) -> Result<bool> {
        use crate::schema::program_authority::dsl::*;

        let conn = &mut self.get_db_conn().await?;

        match program_authority
            .select(is_closed)
            .filter(program_id.eq(program_address))
            .first::<bool>(conn)
            .await
        {
            Ok(closed) => Ok(closed),
            Err(diesel::result::Error::NotFound) => Ok(false),
            Err(e) => {
                error!("Failed to check if program is closed: {}", e);
                Err(ApiError::Diesel(e))
            }
        }
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
            .insert_or_update_program_authority(&program_key, Some(authority), false, Some(false))
            .await;
        assert!(insert_result.is_ok());

        // Test retrieve
        let get_result = client
            .get_program_authority_from_db(&program_key.to_string())
            .await;
        assert!(get_result.is_ok());
        assert_eq!(get_result.unwrap(), Some(authority.to_string()));
    }

    #[tokio::test]
    async fn test_program_frozen_and_closed_status() {
        dotenv::dotenv().ok();
        let db_url = std::env::var("TEST_DATABASE_URL").unwrap();
        let redis_url = std::env::var("TEST_REDIS_URL").unwrap();
        let client = DbClient::new(&db_url, &redis_url);

        // Test Case 1: Normal program (not frozen, not closed)
        let normal_program = Pubkey::new_unique();
        let authority = "normal_authority";

        let insert_result = client
            .insert_or_update_program_authority(
                &normal_program,
                Some(authority),
                false,
                Some(false),
            )
            .await;
        assert!(insert_result.is_ok(), "Failed to insert normal program");

        let is_frozen = client.is_program_frozen(&normal_program.to_string()).await;
        assert!(is_frozen.is_ok());
        assert!(!is_frozen.unwrap(), "Normal program should not be frozen");

        let is_closed = client.is_program_closed(&normal_program.to_string()).await;
        assert!(is_closed.is_ok());
        assert!(!is_closed.unwrap(), "Normal program should not be closed");

        // Test Case 2: Frozen program (frozen, not closed)
        let frozen_program = Pubkey::new_unique();

        let insert_result = client
            .insert_or_update_program_authority(&frozen_program, Some(authority), true, Some(false))
            .await;
        assert!(insert_result.is_ok(), "Failed to insert frozen program");

        let is_frozen = client.is_program_frozen(&frozen_program.to_string()).await;
        assert!(is_frozen.is_ok());
        assert!(
            is_frozen.unwrap(),
            "Frozen program should be marked as frozen"
        );

        let is_closed = client.is_program_closed(&frozen_program.to_string()).await;
        assert!(is_closed.is_ok());
        assert!(!is_closed.unwrap(), "Frozen program should not be closed");

        // Test Case 3: Closed program (not frozen, closed)
        let closed_program = Pubkey::new_unique();

        let insert_result = client
            .insert_or_update_program_authority(&closed_program, None, false, Some(true))
            .await;
        assert!(insert_result.is_ok(), "Failed to insert closed program");

        let is_frozen = client.is_program_frozen(&closed_program.to_string()).await;
        assert!(is_frozen.is_ok());
        assert!(
            !is_frozen.unwrap(),
            "Closed program should not be marked as frozen"
        );

        let is_closed = client.is_program_closed(&closed_program.to_string()).await;
        assert!(is_closed.is_ok());
        assert!(
            is_closed.unwrap(),
            "Closed program should be marked as closed"
        );

        // Test Case 4: Both frozen and closed (edge case)
        let frozen_closed_program = Pubkey::new_unique();

        let insert_result = client
            .insert_or_update_program_authority(&frozen_closed_program, None, true, Some(true))
            .await;
        assert!(
            insert_result.is_ok(),
            "Failed to insert frozen and closed program"
        );

        let is_frozen = client
            .is_program_frozen(&frozen_closed_program.to_string())
            .await;
        assert!(is_frozen.is_ok());
        assert!(
            is_frozen.unwrap(),
            "Frozen and closed program should be marked as frozen"
        );

        let is_closed = client
            .is_program_closed(&frozen_closed_program.to_string())
            .await;
        assert!(is_closed.is_ok());
        assert!(
            is_closed.unwrap(),
            "Frozen and closed program should be marked as closed"
        );

        // Test Case 5: Non-existent program (should return false for both)
        let nonexistent_program = Pubkey::new_unique();

        let is_frozen = client
            .is_program_frozen(&nonexistent_program.to_string())
            .await;
        assert!(is_frozen.is_ok());
        assert!(
            !is_frozen.unwrap(),
            "Non-existent program should not be frozen"
        );

        let is_closed = client
            .is_program_closed(&nonexistent_program.to_string())
            .await;
        assert!(is_closed.is_ok());
        assert!(
            !is_closed.unwrap(),
            "Non-existent program should not be closed"
        );

        // Test Case 6: Update existing program status
        let update_program = Pubkey::new_unique();

        // Initially insert as normal program
        let insert_result = client
            .insert_or_update_program_authority(
                &update_program,
                Some(authority),
                false,
                Some(false),
            )
            .await;
        assert!(
            insert_result.is_ok(),
            "Failed to insert program for update test"
        );

        // Verify initial state
        let is_frozen = client
            .is_program_frozen(&update_program.to_string())
            .await
            .unwrap();
        let is_closed = client
            .is_program_closed(&update_program.to_string())
            .await
            .unwrap();
        assert!(
            !is_frozen && !is_closed,
            "Program should initially be normal"
        );

        // Update to closed
        let update_result = client
            .insert_or_update_program_authority(&update_program, None, false, Some(true))
            .await;
        assert!(update_result.is_ok(), "Failed to update program to closed");

        // Verify updated state
        let is_frozen = client
            .is_program_frozen(&update_program.to_string())
            .await
            .unwrap();
        let is_closed = client
            .is_program_closed(&update_program.to_string())
            .await
            .unwrap();
        assert!(!is_frozen && is_closed, "Program should now be closed");

        // Update to frozen
        let update_result = client
            .insert_or_update_program_authority(&update_program, Some(authority), true, Some(false))
            .await;
        assert!(update_result.is_ok(), "Failed to update program to frozen");

        // Verify final state
        let is_frozen = client
            .is_program_frozen(&update_program.to_string())
            .await
            .unwrap();
        let is_closed = client
            .is_program_closed(&update_program.to_string())
            .await
            .unwrap();
        assert!(
            is_frozen && !is_closed,
            "Program should now be frozen but not closed"
        );
    }
}
