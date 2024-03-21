use anyhow::{bail, Result};
use serde_json::Value;
use solana_client::rpc_client::RpcClient;
use solana_sdk::{
    account_utils::StateMut, bpf_loader_upgradeable::UpgradeableLoaderState, pubkey::Pubkey,
};
use solana_security_txt::SecurityTxt;
use std::{fs::OpenOptions, io::Write};

use crate::{
    api::{
        client::verify_build,
        models::{BuildCommandArgs, SolanaProgramBuildParams},
    },
    db::client::DbClient,
    errors::CrawlerErrors,
    github::GithubClient,
};

// Constants
pub const OUTPUT_FILENAME: &str = "verification_targets.txt";

// Function to write github source_code link to a json file
pub fn write_file(data: &str) -> Result<()> {
    // Open the file with append mode or create it if it doesn't exist
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(OUTPUT_FILENAME)?;

    // Append content followed by a new line
    writeln!(file, "{}", data)?;

    Ok(())
}

pub async fn get_program_security_text(
    pubkey: &Pubkey,
    program_data_address: &Pubkey,
    db: &DbClient,
) -> Result<SecurityTxt> {
    let client = RpcClient::new(crate::RPC_URL);

    // Insert the program into the database
    let program = db
        .insert_program(&pubkey.to_string(), &program_data_address.to_string())
        .await?;

    let program_data_account = client
        .get_account(program_data_address)
        .map_err(|_| CrawlerErrors::ProgramClosed(pubkey.to_string()))?;

    let offset = UpgradeableLoaderState::size_of_programdata_metadata();

    // Get ProgramData Slot from the account
    if let Ok(UpgradeableLoaderState::ProgramData {
        upgrade_authority_address,
        slot,
    }) = program_data_account.state()
    {
        tracing::info!("slot: {}", slot);
        tracing::info!("upgrade_authority_address: {:?}", upgrade_authority_address);

        if program.last_deployed_slot == Some(slot as i64) {
            bail!(CrawlerErrors::ProgramNotUpdated)
        } else {
            db.update_authority_and_slot(&pubkey.to_string(), &upgrade_authority_address, slot)
                .await?;
        }
    } else {
        return Err(CrawlerErrors::ProgramClosed(pubkey.to_string()).into());
    }

    if program_data_account.data.len() < offset {
        return Err(CrawlerErrors::ProgramDataAccountSizeTooSmall.into());
    }

    let program_data = &program_data_account.data[offset..];

    let security_txt = solana_security_txt::find_and_parse(program_data)
        .map_err(CrawlerErrors::SecurityTextNotFound)?;

    Ok(security_txt)
}

// Read file line by line and return it
pub async fn verify_programs(filename: &str) -> Result<()> {
    let file = std::fs::read_to_string(filename)?;
    let lines: Vec<String> = file.lines().map(|s| s.to_string()).collect();
    for line in lines {
        start_verification(&line).await?;
    }

    Ok(())
}

// Split the string by space and get all args
pub fn extract_build_params(input: &BuildCommandArgs) -> SolanaProgramBuildParams {
    let mut params = SolanaProgramBuildParams {
        repository: String::new(),
        program_id: String::new(),
        commit_hash: None,
        lib_name: None,
        bpf_flag: None,
        base_image: None,
        mount_path: None,
        cargo_args: None,
    };

    let mut cargo_args = Vec::new();
    let mut is_cargo_args = false;

    let mut tokens = input.command.split_whitespace().peekable();

    while let Some(token) = tokens.next() {
        if is_cargo_args {
            cargo_args.push(token.to_string());
            continue;
        }

        match token {
            "solana-verify" | "verify-from-repo" => {} // Ignore command and repo
            "--commit-hash" => {
                params.commit_hash = Some(tokens.next().unwrap().to_string());
            }
            "--mount-path" => {
                params.mount_path = Some(tokens.next().unwrap().to_string());
            }
            "--base-image" => {
                params.base_image = Some(tokens.next().unwrap().to_string());
            }
            "--library-name" => {
                params.lib_name = Some(tokens.next().unwrap().to_string());
            }
            "--bpf" => {
                params.bpf_flag = Some(true);
            }
            "--" => {
                is_cargo_args = true;
            }
            _ => {
                // unknown flag
            }
        }
    }
    params.program_id = input.program_id.to_string();
    params.repository = input.repo.to_string();
    if is_cargo_args {
        params.cargo_args = Some(cargo_args);
    }
    params
}

fn extract_owner_and_repo(url: &str) -> Option<(String, String)> {
    let parts: Vec<&str> = url.trim_end_matches('/').split('/').collect();
    if parts.len() >= 3 && parts[0] == "https:" && parts[1].is_empty() && parts[2] == "github.com" {
        Some((parts[3].to_string(), parts[4].to_string()))
    } else {
        None
    }
}

// Start Verification and get Result
pub async fn start_verification(source_code: &str) -> Result<()> {
    // if source_code is end with / remove it
    let (owner, repo) = extract_owner_and_repo(source_code)
        .ok_or_else(|| anyhow::format_err!("Invalid source code URL."))?;

    let github_client = GithubClient::new(&owner, &repo);
    let json_params = github_client.get_verification_json().await?;

    for (key, arr) in json_params {
        let params = if let Value::Array(arr) = arr.clone() {
            arr.iter()
                .map(|v| v.as_str().unwrap_or_default())
                .collect::<Vec<&str>>()
                .join(" ")
        } else {
            "".to_string()
        };
        let params = BuildCommandArgs {
            repo: source_code.to_string(),
            program_id: key.to_string(),
            command: params,
        };

        let build_params = extract_build_params(&params);
        verify_build(build_params).await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn test_extract_build_params() {
        let github_repo = "https://github.com/Ellipsis-Labs/phoenix-v1";
        let client = GithubClient::new("Ellipsis-Labs", "phoenix-v1");
        let json_params = client.get_verification_json().await.unwrap();

        for (key, arr) in json_params {
            let params = if let Value::Array(arr) = arr.clone() {
                arr.iter()
                    .map(|v| v.as_str().unwrap_or_default())
                    .collect::<Vec<&str>>()
                    .join(" ")
            } else {
                "".to_string()
            };
            let params = BuildCommandArgs {
                repo: github_repo.to_string(),
                program_id: key.to_string(),
                command: params,
            };

            let build_params = extract_build_params(&params);

            assert_eq!(build_params.repository, github_repo);
            assert_eq!(build_params.program_id, key.to_string());
            assert!(build_params.commit_hash.is_none());
            assert!(build_params.lib_name.is_none());
            assert!(build_params.bpf_flag.is_none());
        }
    }
}
