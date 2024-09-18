use super::DbClient;
use crate::db::models::{SolanaProgramBuild, SolanaProgramBuildParams};
use crate::Result;
use diesel::{expression_methods::ExpressionMethods, query_dsl::QueryDsl};
use diesel_async::RunQueryDsl;

impl DbClient {
    pub async fn insert_build_params(&self, payload: &SolanaProgramBuild) -> Result<usize> {
        use crate::schema::solana_program_builds::dsl::*;

        let conn = &mut self.db_pool.get().await?;
        diesel::insert_into(solana_program_builds)
            .values(payload)
            .execute(conn)
            .await
            .map_err(Into::into)
    }

    pub async fn check_for_duplicate(
        &self,
        payload: &SolanaProgramBuildParams,
    ) -> Result<SolanaProgramBuild> {
        use crate::schema::solana_program_builds::dsl::*;

        let conn = &mut self.db_pool.get().await?;

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

        query
            .first::<SolanaProgramBuild>(conn)
            .await
            .map_err(Into::into)
    }

    pub async fn get_build_params(&self, program_address: &str) -> Result<SolanaProgramBuild> {
        use crate::schema::solana_program_builds::dsl::*;

        let conn = &mut self.db_pool.get().await?;
        solana_program_builds
            .filter(crate::schema::solana_program_builds::program_id.eq(program_address))
            .first::<SolanaProgramBuild>(conn)
            .await
            .map_err(Into::into)
    }
}
