use std::str::FromStr;
use std::time::Duration;

use solana_account_decoder::UiAccountEncoding;
use solana_client::rpc_filter::RpcFilterType;
use solana_client::{
    rpc_client::RpcClient,
    rpc_config::{RpcAccountInfoConfig, RpcProgramAccountsConfig},
};
use solana_sdk::account_utils::StateMut;
use solana_sdk::bpf_loader_upgradeable::UpgradeableLoaderState;
use solana_sdk::pubkey::Pubkey;

use crate::db::client::DbClient;
use crate::errors;

// Crawl the mainnet programs and write them to a file
pub async fn crawl_mainnet_programs(db: &DbClient) {
    let timeout = Duration::from_secs(3600);

    let client = RpcClient::new_with_timeout(crate::RPC_URL, timeout);

    // Only bpf_loader_upgradeable programs have support for security.txt
    let pubkey = Pubkey::from_str("BPFLoaderUpgradeab1e11111111111111111111111").unwrap();

    // filter account with size 36
    let filters = Some(vec![RpcFilterType::DataSize(36)]);

    let response = client.get_program_accounts_with_config(
        &pubkey,
        RpcProgramAccountsConfig {
            filters,
            account_config: RpcAccountInfoConfig {
                encoding: Some(UiAccountEncoding::Base64),
                ..RpcAccountInfoConfig::default()
            },
            ..RpcProgramAccountsConfig::default()
        },
    );

    let response = response.unwrap();

    tracing::info!("Found {} accounts", response.len());

    for account in response {
        // test
        if let Ok(UpgradeableLoaderState::Program {
            programdata_address,
        }) = account.1.state()
        {
            tracing::info!("Fetching : {:?}", programdata_address);
            let result =
                crate::helper::get_program_security_text(&account.0, &programdata_address, db)
                    .await;
            // Check if security text is available
            if let Ok(security_txt) = result {
                // Check if source code is available
                if let Some(source_code) = security_txt.source_code {
                    tracing::info!("{}'s Source code: {}", account.0, source_code);
                    let _ = crate::helper::write_file(&source_code);
                    db.update_program_info(
                        &account.0.to_string(),
                        &source_code,
                        &security_txt.name,
                    )
                    .await
                    .unwrap();
                } else {
                    tracing::error!(
                        "Failed to get source_code from security.txt for pubkey: {}",
                        account.0
                    );
                }
            } else {
                tracing::error!("Failed to get security text for pubkey: {}", account.0);

                // Match the error and update the status in the database
                if let Err(err) = result {
                    crate::errors::handle_crawler_errors(
                        err.downcast_ref::<errors::CrawlerErrors>(),
                        db,
                        &account.0,
                    )
                    .await;
                }
            }
        } else {
            tracing::error!(
                "Failed to get program data address for pubkey: {}",
                account.0.to_string()
            );
        }
    }
}
