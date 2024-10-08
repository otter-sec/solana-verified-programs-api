use crate::{db::{client::DbClient, misc::update_program_status_and_security_txt_status}, security_txt::parser::SecurityTxtError};
use solana_client::client_error::ClientError;
use solana_sdk::{instruction::InstructionError, pubkey::Pubkey};
use thiserror::Error;

// CrawlerErrors
#[derive(Error, Debug)]
pub enum CrawlerErrors {
    #[error("Failed to fetch program account: {0}")]
    FailedToFetchProgramAccount(#[from] ClientError),
    #[error("Failed to get program data offset: {0}")]
    FailedToGetProgramDataOffset(#[from] InstructionError),
    #[error("Program data account size too small")]
    ProgramDataAccountSizeTooSmall,
    #[error("Failed to find and parse security.txt: {0}")]
    SecurityTextNotFound(#[from] SecurityTxtError),
    #[error("Program {0} has been closed")]
    ProgramClosed(String),
    #[error("Program not updated since last check")]
    ProgramNotUpdated,
    #[error("Default branch not found")]
    DefaultBranchNotFound,
    #[error("Invalid Json File contents")]
    InvalidJsonFileContents,
}

// Function to hanle the error cases when fetching the program account's security.txt
pub async fn handle_crawler_errors(err: Option<&CrawlerErrors>, db: &DbClient, pubkey: &Pubkey) {
    let (mut is_program_account_closed, mut has_succeeded) = (false, true);

    if let Some(err) = err {
        match err {
            CrawlerErrors::FailedToFetchProgramAccount(e) => {
                tracing::error!("Failed to fetch program account: {}", e);
                has_succeeded = false;
            }
            CrawlerErrors::FailedToGetProgramDataOffset(e) => {
                tracing::error!("Failed to get program data offset: {}", e);
            }
            CrawlerErrors::ProgramDataAccountSizeTooSmall => {
                tracing::error!("Program data account size too small");
            }
            CrawlerErrors::SecurityTextNotFound(e) => {
                tracing::error!("Failed to find and parse security.txt: {}", e);
            }
            CrawlerErrors::ProgramClosed(addr) => {
                tracing::error!("Program {} has been closed", addr);
                is_program_account_closed = true;
            }
            _ => {
                return;
            }
        }
    } else {
        is_program_account_closed = false;
        has_succeeded = false;
    }

    update_program_status_and_security_txt_status(
        db,
        &pubkey.to_string(),
        has_succeeded,
        false,
        is_program_account_closed,
    )
    .await;
}
