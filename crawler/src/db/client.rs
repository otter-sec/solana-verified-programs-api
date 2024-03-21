use crate::db::models::MainnetProgram;
use anyhow::Result;
use diesel::{expression_methods::ExpressionMethods, query_dsl::QueryDsl};
use diesel_async::pooled_connection::AsyncDieselConnectionManager;
use diesel_async::RunQueryDsl;
use diesel_async::{pooled_connection::deadpool::Pool, AsyncPgConnection};
use solana_sdk::pubkey::Pubkey;

#[derive(Clone)]
pub struct DbClient {
    pub db_pool: Pool<AsyncPgConnection>,
}

impl DbClient {
    pub fn new(db_url: &str) -> Self {
        let config = AsyncDieselConnectionManager::<diesel_async::AsyncPgConnection>::new(db_url);
        let postgres_pool = Pool::builder(config)
            .build()
            .expect("Failed to create DB Pool");

        Self {
            db_pool: postgres_pool,
        }
    }

    pub async fn insert_program(
        &self,
        program_addrs: &str,
        prgram_data_addr: &str,
    ) -> Result<MainnetProgram> {
        use crate::schema::mainnet_programs::dsl::*;
        use diesel::insert_into;

        let conn = &mut self.db_pool.get().await?;
        let inserted = insert_into(mainnet_programs)
            .values((
                project_name.eq(None::<String>),
                program_address.eq(program_addrs),
                buffer_address.eq(prgram_data_addr),
                github_repo.eq(None::<String>),
                has_security_txt.eq(false),
                is_closed.eq(false),
                is_success.eq(false),
                is_processed.eq(false),
                updated_at.eq(chrono::Utc::now().naive_utc()),
                last_deployed_slot.eq(None::<i64>),
                update_authority.eq(None::<String>),
            ))
            .on_conflict(program_address)
            .do_update()
            .set(is_processed.eq(false))
            .get_result::<MainnetProgram>(conn)
            .await?;

        Ok(inserted)
    }

    pub async fn update_authority_and_slot(
        &self,
        program_id: &str,
        authority: &Option<Pubkey>,
        slot: u64,
    ) -> Result<()> {
        use crate::schema::mainnet_programs::dsl::*;

        let conn = &mut self.db_pool.get().await?;

        match authority {
            Some(authority) => {
                diesel::update(mainnet_programs.filter(program_address.eq(program_id)))
                    .set((
                        update_authority.eq(authority.to_string()),
                        last_deployed_slot.eq(slot as i64),
                    ))
                    .execute(conn)
                    .await?;
            }
            None => {
                diesel::update(mainnet_programs.filter(program_address.eq(program_id)))
                    .set(last_deployed_slot.eq(slot as i64))
                    .execute(conn)
                    .await?;
            }
        }

        Ok(())
    }

    // Update github_repo and project_name with program address
    pub async fn update_program_info(
        &self,
        program_id: &str,
        github_url: &str,
        name: &str,
    ) -> Result<()> {
        use crate::schema::mainnet_programs::dsl::*;
        let conn = &mut self.db_pool.get().await?;
        diesel::update(mainnet_programs.filter(program_address.eq(program_id)))
            .set((
                github_repo.eq(github_url),
                project_name.eq(name),
                has_security_txt.eq(true),
                is_success.eq(true),
            ))
            .execute(conn)
            .await?;

        Ok(())
    }

    // Update status of the program
    pub async fn update_program_status(&self, program_id: &str, status: bool) -> Result<()> {
        use crate::schema::mainnet_programs::dsl::*;
        let conn = &mut self.db_pool.get().await?;
        diesel::update(mainnet_programs.filter(program_address.eq(program_id)))
            .set(is_success.eq(status))
            .execute(conn)
            .await?;

        Ok(())
    }

    // Set is_closed status of the program
    pub async fn set_is_closed(&self, program_id: &str, status: bool) -> Result<()> {
        use crate::schema::mainnet_programs::dsl::*;
        let conn = &mut self.db_pool.get().await?;
        diesel::update(
            mainnet_programs
                .filter(program_address.eq(program_id))
                .filter(is_closed.eq(false)),
        )
        .set(is_closed.eq(status))
        .execute(conn)
        .await?;

        Ok(())
    }

    // Update security_txt status of the program
    pub async fn update_security_txt_status(&self, program_id: &str, status: bool) -> Result<()> {
        use crate::schema::mainnet_programs::dsl::*;
        let conn = &mut self.db_pool.get().await?;
        diesel::update(mainnet_programs.filter(program_address.eq(program_id)))
            .set(has_security_txt.eq(status))
            .execute(conn)
            .await?;

        Ok(())
    }
}
