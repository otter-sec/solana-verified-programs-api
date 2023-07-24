use dotenv::dotenv;
use routes::create_router;
use std::env;

extern crate diesel;
extern crate tracing;

mod db;
mod errors;
mod models;
mod routes;
mod schema;
mod utils;

#[tokio::main]
async fn main() {
    dotenv().ok();
    tracing_subscriber::fmt::init();
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL not set in .env file");

    let db_client = db::DbClient::new(&database_url);
    let app = create_router(db_client);

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], 3000));
    tracing::info!("Listening on {}", addr);

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}
