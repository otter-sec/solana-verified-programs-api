use borsh::{ BorshDeserialize, BorshSerialize };
use solana_sdk::pubkey::Pubkey;
use base64::Engine;
use reqwest::Client;
use serde::{ Deserialize, Serialize };
use serde_json::json;
use crate::{ errors::ApiError, Result };
use std::env;

#[derive(Serialize, Deserialize, Debug)]
struct Params {
    encoding: String,
    filters: Vec<Filter>,
}

#[derive(Serialize, Deserialize, Debug)]
struct Filter {
    memcmp: Memcmp,
}

#[derive(Serialize, Deserialize, Debug)]
struct Memcmp {
    offset: u64,
    bytes: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct RpcRequest {
    jsonrpc: String,
    id: u64,
    method: String,
    params: Vec<serde_json::Value>,
}

#[derive(Serialize, Deserialize, Debug)]
struct Account {
    data: (String, String),
    executable: bool,
    lamports: u64,
    owner: String,
    #[serde(rename = "rentEpoch")]
    rent_epoch: u64,
    space: u64,
}

#[derive(Serialize, Deserialize, Debug)]
struct ResultEntry {
    account: Account,
    pubkey: String,
}

#[derive(Serialize, Deserialize, Debug)]
struct RpcResponse {
    jsonrpc: String,
    id: u64,
    result: Option<Vec<ResultEntry>>,
}

#[derive(BorshDeserialize, BorshSerialize, Debug)]
pub struct OtterBuildParams {
    pub address: Pubkey,
    pub signer: Pubkey,
    pub version: String,
    pub git_url: String,
    pub commit: String,
    pub args: Vec<String>,
    bump: u8,
}

pub async fn get_otter_verify_params(program_id: &str) -> Result<OtterBuildParams> {
    let rpc_url = env
        ::var("RPC_URL")
        .unwrap_or_else(|_| "https://api.mainnet-beta.solana.com".to_string());

    let client = Client::new();

    let params = Params {
        encoding: "base64".to_string(),
        filters: vec![Filter {
            memcmp: Memcmp {
                offset: 8,
                bytes: program_id.to_string(),
            },
        }],
    };

    let request = RpcRequest {
        jsonrpc: "2.0".to_string(),
        id: 1,
        method: "getProgramAccounts".to_string(),
        params: vec![
            json!(program_id),
            serde_json
                ::to_value(params)
                .map_err(|_| ApiError::Custom("Failed to serialize params".to_string()))?
        ],
    };

    println!("{:?}", request);

    let response = client
        .post(rpc_url)
        .json(&request)
        .send().await
        .map_err(|_| {
            ApiError::Custom(
                "Failed to send request to get Otter Verify params from mainnet".to_string()
            )
        })?;

    let response_json: RpcResponse = response
        .json().await
        .map_err(|_| ApiError::Custom("Failed to parse response from mainnet".to_string()))?;

    if let Some(result) = response_json.result {
        if let Some(entry) = result.into_iter().next() {
            let data = base64::prelude::BASE64_STANDARD
                .decode(entry.account.data.0)
                .map_err(|_| {
                    ApiError::Custom("Failed to decode data from mainnet".to_string())
                })?;
            let anchor_account: OtterBuildParams = BorshDeserialize::try_from_slice(
                &data[8..]
            ).map_err(|_| { ApiError::Custom("Failed to decode anchor account".to_string()) })?;
            tracing::info!("Anchor Account: {:?}", anchor_account);
            return Ok(anchor_account);
        }
        Err(ApiError::Custom("Failed to find Otter Verify params".to_string()))
    } else {
        tracing::info!("No results found");
        Err(ApiError::Custom("Failed to find Otter Verify params".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_get_on_chain_hash() {
        let program_id = "verifycLy8mB96wd9wqq3WDXQwM4oU6r42Th37Db9fC";
        let data = get_otter_verify_params(program_id).await;
        assert!(data.is_ok());
        let params = data.unwrap();
        assert!(params.address.to_string() == "verifycLy8mB96wd9wqq3WDXQwM4oU6r42Th37Db9fC");
        assert!(params.signer.to_string() == "9VWiUUhgNoRwTH5NVehYJEDwcotwYX3VgW4MChiHPAqU");
    }
}
