use dotenv::dotenv;
use routes::create_router;
use state::AppState;
use std::env;

extern crate diesel;
extern crate tracing;

mod errors;
mod models;
mod operations;
mod routes;
mod schema;
mod state;
mod utils;

#[tokio::main]
async fn main() {
    dotenv().ok();
    tracing_subscriber::fmt::init();
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL not set in .env file");

    let db_client = state::DbClient::new(&database_url);

    let app_state = AppState { db_client };

    let app = create_router(app_state);

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], 3000));
    tracing::info!("Listening on {}", addr);

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}
