use std::sync::Arc;

use diesel::{
    r2d2::{ConnectionManager, Pool},
    PgConnection,
};

#[derive(Clone)]
pub struct AppState {
    pub db_pool: Arc<Pool<ConnectionManager<PgConnection>>>,
}
