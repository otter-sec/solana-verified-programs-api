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

#[tokio::main]
async fn main() {
    dotenv().ok();
    let rpc_url =
        env::var("RPC_URL").unwrap_or_else(|_| "https://api.mainnet-beta.solana.com".to_string());
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
    crate::crawler::crawl_mainnet_programs(&db_client, &rpc_url).await;

    // Verify the programs
    let _ = helper::verify_programs(helper::OUTPUT_FILENAME).await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::models::BuildCommandArgs;
    use crate::helper::extract_build_params;
    use serde_json::Value;

    #[tokio::test]
    async fn test_build_program() {
        let args = api::models::BuildCommandArgs {
            repo: "https://github.com/Squads-Protocol/squads-mpl".to_string(),
            program_id: "SMPLecH534NA9acpos4G6x7uf3LWbCAwZQE9e8ZekMu".to_string(),
            command: vec![
                "--commit-hash".to_string(),
                "c95b7673d616c377a349ca424261872dfcf8b19d".to_string(),
                "--library-name".to_string(),
                "squads_mpl".to_string(),
                "--bpf".to_string(),
            ],
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

    #[tokio::test]
    async fn test_json_verification() {
        let json_str = r#"{
        "dRiftyHA39MWEi3m9aunc5MzRF1JYuBsbn6VPcn33UH": [
            "--commit-hash",
            "8d2cd726afdc800f89c841ff3cf1968980719df0",
            "--library-name",
            "drift"
        ]
    }"#;
        let json: Value = serde_json::from_str(json_str).unwrap();
        let map = json.as_object().unwrap();
        for (key, arr) in map {
            let params = if let Value::Array(arr) = arr {
                arr.iter()
                    .map(|v| v.as_str().unwrap_or_default().to_owned())
                    .collect::<Vec<String>>()
            } else {
                Vec::new()
            };
            let params = BuildCommandArgs {
                repo: "https://github.com/drift-labs/protocol-v2/".to_string(),
                program_id: key.to_string(),
                command: params,
            };

            let build_params = extract_build_params(&params);
            assert!(build_params.commit_hash.is_some());
        }
    }
}
