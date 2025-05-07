use crate::{
    db::{
        models::{VerifiedProgram, VerifiedProgramStatusResponse},
        DbClient,
    },
    Result,
};
use diesel::QueryDsl;
use diesel_async::RunQueryDsl;
use diesel::ExpressionMethods;
use tracing::{error, info};

const PER_PAGE: i64 = 20;

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

    /// Retrieves a page of verified programs from the database
    ///
    /// Returns a list of VerifiedProgram structs
    ///
    ///  
    pub async fn get_verified_program_ids_page(&self, page: i64) -> Result<Vec<String>> {
        use crate::schema::verified_programs::dsl::*;

        // Ensure page is valid
        let page = if page > 0 { page } else { 1 };
        let offset = (page - 1) * PER_PAGE;
        
        let conn = &mut self.get_db_conn().await?;

        let total_count = verified_programs
            .filter(is_verified.eq(true))
            .distinct_on(program_id)
            .count()
            .get_result::<i64>(conn)
            .await?;
        tracing::info!("Total count of verified programs: {}", total_count);

        
        verified_programs
            .filter(is_verified.eq(true))
            .select(program_id)
            .distinct_on(program_id)
            .order_by((program_id, id))
            .limit(PER_PAGE)
            .offset(offset)
            .load::<String>(conn)
            .await
            .map_err(|e| {
                error!("Failed to fetch verified programs: {}", e);
                e.into()
            })
    }

    pub async fn get_verification_status_all(&self) -> Result<Vec<VerifiedProgramStatusResponse>> {
        let all_verified_programs = self.get_verified_programs().await?;

        let mut programs_status_all = Vec::new();

        for program in all_verified_programs {
            info!(
                "Checking verification status for program: {}",
                program.program_id
            );
            match self
                .clone()
                .check_is_verified(program.program_id.to_owned(), None)
                .await
            {
                Ok(result) => {
                    let status_message = if result.is_verified {
                        "On chain program verified"
                    } else {
                        "On chain program not verified"
                    };

                    info!("Program {} status: {}", program.program_id, status_message);
                    programs_status_all.push(VerifiedProgramStatusResponse {
                        program_id: program.program_id.to_owned(),
                        is_verified: result.is_verified,
                        message: status_message.to_string(),
                        on_chain_hash: result.on_chain_hash,
                        executable_hash: result.executable_hash,
                        last_verified_at: result.last_verified_at,
                        repo_url: result.repo_url,
                        commit: result.commit,
                    });
                }
                Err(err) => {
                    error!(
                        "Failed to get verification status for program {}: {}",
                        program.program_id, err
                    );
                }
            }
        }

        Ok(programs_status_all)
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
