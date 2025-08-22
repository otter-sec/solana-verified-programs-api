use super::models::SolanaProgramBuildParams;
use super::DbClient;
use crate::{db::models::SolanaProgramBuild, errors::ApiError, Result};
use diesel::{expression_methods::ExpressionMethods, query_dsl::QueryDsl};
use diesel_async::RunQueryDsl;
use tracing::{error, info};

/// DbClient helper functions for SolanaProgramBuilds table
impl DbClient {
    /// Insert build params for a program
    pub async fn insert_build_params(&self, payload: &SolanaProgramBuild) -> Result<usize> {
        use crate::schema::solana_program_builds::dsl::*;

        let conn = &mut self.get_db_conn().await?;

        info!("Inserting build params for program: {}", payload.program_id);
        diesel::insert_into(solana_program_builds)
            .values(payload)
            .execute(conn)
            .await
            .map_err(|e| {
                error!("Failed to insert build params: {}", e);
                ApiError::Diesel(e)
            })
    }

    /// Check for duplicate build params for a program
    pub async fn check_for_duplicate(
        &self,
        data: &SolanaProgramBuildParams,
        pda_signer: String,
    ) -> Result<SolanaProgramBuild> {
        use crate::schema::solana_program_builds::dsl::*;

        let conn = &mut self.get_db_conn().await?;
        let payload = &data;

        info!(
            "Checking for duplicate build with program_id: {}",
            payload.program_id
        );

        let mut query = solana_program_builds.into_boxed();

        query = query.filter(program_id.eq(payload.program_id.to_owned()));
        query = query.filter(repository.eq(payload.repository.to_owned()));

        if let Some(hash) = &payload.commit_hash {
            query = query.filter(commit_hash.eq(hash));
        }

        if let Some(lib) = &payload.lib_name {
            query = query.filter(lib_name.eq(lib));
        }

        if let Some(bpf) = &payload.bpf_flag {
            query = query.filter(bpf_flag.eq(bpf));
        }

        if let Some(base) = &payload.base_image {
            query = query.filter(base_docker_image.eq(base));
        }

        if let Some(mount) = &payload.mount_path {
            query = query.filter(mount_path.eq(mount));
        }

        if let Some(args) = payload.cargo_args.clone() {
            query = query.filter(cargo_args.eq(args));
        }

        query = query.filter(signer.eq(pda_signer));

        query
            .order(created_at.desc())
            .first::<SolanaProgramBuild>(conn)
            .await
            .map_err(|e| {
                error!("Failed to check for duplicate build: {}", e);
                ApiError::Diesel(e)
            })
    }

    /// Get the latest build params for a program by its program address
    pub async fn get_build_params(&self, program_address: &str) -> Result<SolanaProgramBuild> {
        use crate::schema::solana_program_builds::dsl::*;

        let conn = &mut self.get_db_conn().await?;

        info!("Fetching build params for program: {}", program_address);
        solana_program_builds
            .filter(program_id.eq(program_address))
            .order(created_at.desc())
            .first::<SolanaProgramBuild>(conn)
            .await
            .map_err(|e| {
                error!("Failed to get build params: {}", e);
                ApiError::Diesel(e)
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[tokio::test]
    async fn test_build_params_operations() {
        dotenv::dotenv().ok();
        let db_url = std::env::var("TEST_DATABASE_URL").unwrap();
        let redis_url = std::env::var("TEST_REDIS_URL").unwrap();
        let client = DbClient::new(&db_url, &redis_url);

        // Create test build params
        let build = SolanaProgramBuild {
            id: "test_id".to_string(),
            repository: "test_repo".to_string(),
            program_id: "test_program".to_string(),
            commit_hash: Some("test_hash".to_string()),
            lib_name: None,
            base_docker_image: None,
            mount_path: None,
            cargo_args: None,
            bpf_flag: true,
            created_at: Utc::now().naive_utc(),
            status: "in_progress".to_string(),
            signer: Some("test_signer".to_string()),
            arch: None,
        };

        // Test insert
        let insert_result = client.insert_build_params(&build).await;
        assert!(insert_result.is_ok());

        // Test retrieve
        let get_result = client.get_build_params(&build.program_id).await;
        assert!(get_result.is_ok());
    }
}
