use diesel::{
    expression_methods::ExpressionMethods,
    query_dsl::QueryDsl,
    r2d2::{ConnectionManager, Pool},
    PgConnection, RunQueryDsl,
};
use std::sync::Arc;

use crate::models::{SolanaProgramBuild, VerifiedProgram};
use crate::schema;

#[derive(Clone)]
pub struct AppState {
    pub db_client: DbClient,
}

#[derive(Clone)]
pub struct DbClient {
    pub db_pool: Arc<Pool<ConnectionManager<PgConnection>>>,
}

impl DbClient {
    pub fn new(db_url: &str) -> Self {
        Self {
            db_pool: Arc::new(
                Pool::builder()
                    .build(ConnectionManager::<PgConnection>::new(db_url))
                    .expect("Failed to create pool."),
            ),
        }
    }

    pub async fn insert_or_update_build(
        &self,
        payload: &SolanaProgramBuild,
    ) -> Result<(), diesel::result::Error> {
        let conn = &mut self.db_pool.get().unwrap();

        diesel::insert_into(schema::solana_program_builds::table)
            .values(payload)
            .on_conflict(schema::solana_program_builds::program_id)
            .do_update()
            .set(payload)
            .execute(conn)?;

        Ok(())
    }

    pub async fn insert_or_update_verified_build(
        &self,
        payload: &VerifiedProgram,
    ) -> Result<(), diesel::result::Error> {
        let conn = &mut self.db_pool.get().unwrap();

        diesel::insert_into(schema::verified_programs::table)
            .values(payload)
            .on_conflict(schema::verified_programs::program_id)
            .do_update()
            .set(payload)
            .execute(conn)?;

        Ok(())
    }

    pub async fn get_build_params(
        &self,
        program_address: &String,
    ) -> Result<SolanaProgramBuild, diesel::result::Error> {
        let conn = &mut self.db_pool.get().unwrap();
        let res = schema::solana_program_builds::table
            .filter(schema::solana_program_builds::program_id.eq(program_address))
            .first::<SolanaProgramBuild>(conn)?;

        Ok(res)
    }

    pub async fn get_verified_build(
        &self,
        program_address: &String,
    ) -> Result<VerifiedProgram, diesel::result::Error> {
        let conn = &mut self.db_pool.get().unwrap();
        let res = schema::verified_programs::table
            .filter(schema::verified_programs::program_id.eq(program_address))
            .first::<VerifiedProgram>(conn)?;

        Ok(res)
    }
}
