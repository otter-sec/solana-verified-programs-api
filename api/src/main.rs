use config::Config;
use dotenv::dotenv;
use std::net::SocketAddr;

extern crate diesel;
extern crate tracing;

mod api;
mod config;
mod db;
mod errors;
mod schema;
mod services;

pub type Result<T> = std::result::Result<T, errors::ApiError>;

#[macro_use]
extern crate lazy_static;

lazy_static! {
    pub static ref CONFIG: Config = load_config();
}

pub fn load_config() -> Config {
    dotenv().ok();
    envy::from_env::<Config>().expect("Failed to load configuration")
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    let database_url = CONFIG.database_url.clone();
    let redis_url = CONFIG.redis_url.clone();

    let db_client = db::DbClient::new(&database_url, &redis_url);
    let app = api::initialize_router(db_client);

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    tracing::info!("Listening on {}", addr);

    axum::Server::bind(&addr)
        .serve(app.into_make_service_with_connect_info::<SocketAddr>())
        .await
        .unwrap();
}
