//! On-chain program hash retrieval.

use crate::{
    errors::{ApiError, Result},
    services::onchain::program_authority_retriever::get_program_state,
    validation::Address,
};
use solana_client::nonblocking::rpc_client::RpcClient;

/// Retrieves the on-chain hash for a Solana program by fetching the
/// program-data account and hashing the bytes inline.
///
/// Returns `Err` with `"Program appears to be closed"` when the program
/// has no on-chain bytes -- callers pattern-match on that message.
pub async fn get_on_chain_hash(rpc: &RpcClient, program_id: &Address) -> Result<String> {
    let state = get_program_state(rpc, program_id.as_pubkey()).await?;
    if state.is_closed {
        return Err(ApiError::NotFound(
            "Program appears to be closed".to_string(),
        ));
    }
    state
        .executable_hash
        .ok_or_else(|| ApiError::Custom("Failed to extract on-chain program hash".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::str::FromStr;

    fn rpc() -> RpcClient {
        RpcClient::new("https://api.mainnet-beta.solana.com".to_string())
    }

    #[tokio::test]
    #[ignore = "hits mainnet RPC"]
    async fn test_get_on_chain_hash() {
        let program_id = Address::from_str("verifycLy8mB96wd9wqq3WDXQwM4oU6r42Th37Db9fC").unwrap();
        let hash = get_on_chain_hash(&rpc(), &program_id).await.expect("hash");
        assert_eq!(
            hash,
            "c117c3610fca94c5be64eed41e4f2f6783a38b493b245207f3d7e3d7a63ae8e0"
        );
    }

    #[tokio::test]
    #[ignore = "hits mainnet RPC"]
    async fn test_get_on_chain_hash_closed_program() {
        let program_id = Address::from_str("2gFsaXeN9jngaKbQvZsLwxqfUrT2n4WRMraMpeL8NwZM").unwrap();
        let err = get_on_chain_hash(&rpc(), &program_id).await.unwrap_err();
        assert!(
            err.to_string().contains("Program appears to be closed"),
            "Error should indicate program is closed: {err}"
        );
    }
}
