use chrono::NaiveDateTime;
use diesel::prelude::*;

#[derive(Queryable, Selectable, Insertable, PartialEq, Debug, AsChangeset)]
#[diesel(table_name = crate::schema::mainnet_programs)]
#[diesel(check_for_backend(diesel::pg::Pg))]
pub struct MainnetProgram {
    pub id: i32,
    pub project_name: Option<String>,
    pub program_address: String,
    pub buffer_address: String,
    pub github_repo: Option<String>,
    pub has_security_txt: bool,
    pub is_closed: bool,
    pub is_success: bool,
    pub is_processed: bool,
    pub updated_at: NaiveDateTime,
    pub last_deployed_slot: Option<i64>,
    pub update_authority: Option<String>,
}
