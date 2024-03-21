use dotenv::dotenv;
use routes::create_router;
use std::env;
use std::net::SocketAddr;

extern crate diesel;
extern crate tracing;

mod builder;
mod db;
mod errors;
mod models;
mod routes;

mod schema {
    include!("../../schema.rs");
}

pub type Result<T> = std::result::Result<T, errors::ApiError>;

#[tokio::main]
async fn main() {
    dotenv().ok();
    tracing_subscriber::fmt::init();
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL not set in .env file");
    let redis_url = env::var("REDIS_URL").expect("REDIS_URL not set in .env file");

    let db_client = db::DbClient::new(&database_url, &redis_url);
    let app = create_router(db_client);

    let addr = SocketAddr::from(([0, 0, 0, 0], 3000));
    tracing::info!("Listening on {}", addr);

    axum::Server::bind(&addr)
        .serve(app.into_make_service_with_connect_info::<SocketAddr>())
        .await
        .unwrap();
}
