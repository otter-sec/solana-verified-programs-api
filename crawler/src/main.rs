use dotenv::dotenv;
use std::env;
use std::fs::OpenOptions;

mod api;
mod crawler;
mod db;
mod errors;
mod github;
mod helper;
mod schema;

const RPC_URL: &str = "https://api.mainnet-beta.solana.com";

#[tokio::main]
async fn main() {
    dotenv().ok();
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let db_client = db::client::DbClient::new(&database_url);

    tracing_subscriber::fmt()
        .pretty()
        .without_time()
        .with_file(false)
        .with_line_number(false)
        .with_target(false)
        .init();

    OpenOptions::new()
        .write(true)
        .truncate(true)
        .create(true)
        .open(helper::OUTPUT_FILENAME)
        .unwrap();

    // Crawl the mainnet programs and write github source links to a file
    crate::crawler::crawl_mainnet_programs(&db_client).await;

    // Verify the programs
    let _ = helper::verify_programs(helper::OUTPUT_FILENAME).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    // use solana_sdk::pubkey::Pubkey;
    // use std::str::FromStr;

    // #[test]
    // fn test_get_program_security_text() {
    //     let pubkey = Pubkey::from_str("CSwAp3hdedZJBmhWMjv8BJ7anTLMQ2hBqKdnXV5bB3Nz").unwrap();
    //     let result = helper::get_program_security_text(&pubkey);

    //     assert!(result.is_ok());

    //     let source_link = result.unwrap().source_code.unwrap();
    //     assert_eq!(source_link, "https://github.com/rally-dfs/canonical-swap");
    // }

    // #[test]
    // fn test_get_program_security_text_invalid() {
    //     let pubkey = Pubkey::from_str("metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s").unwrap();
    //     let result = helper::get_program_security_text(&pubkey);
    //     assert!(result.is_err());
    // }

    #[tokio::test]
    async fn test_build_program() {
        let args = api::models::BuildCommandArgs {
            repo: "https://github.com/Squads-Protocol/squads-mpl".to_string(),
            program_id: "SMPLecH534NA9acpos4G6x7uf3LWbCAwZQE9e8ZekMu".to_string(),
            command: "--commit-hash c95b7673d616c377a349ca424261872dfcf8b19d -um --library-name squads_mpl --bpf ".to_string(),
        };
        let build_params = helper::extract_build_params(&args);

        let result = api::client::verify_build(build_params).await;
        assert!(result.is_ok());
    }

    #[tokio::test] // Need to Hard code the github url to test
    async fn test_verification() {
        helper::start_verification("https://github.com/Ellipsis-Labs/phoenix-v1/")
            .await
            .unwrap();
    }
}
