use crate::{errors::ApiError, Result, CONFIG};
use solana_client::nonblocking::rpc_client::RpcClient;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

/// RPC Manager that handles key rotation for API time limit errors
#[derive(Debug)]
pub struct RpcManager {
    /// List of RPC URLs for rotation
    rpc_urls: Vec<String>,
    /// Current index in the rotation
    current_index: Arc<RwLock<usize>>,
}

impl RpcManager {
    /// Create a new RPC manager with URLs from config
    pub fn new() -> Self {
        let rpc_urls = if let Some(urls_str) = &CONFIG.rpc_urls {
            // Parse comma-separated URLs and trim whitespace
            urls_str
                .split(',')
                .map(|url| url.trim().to_string())
                .filter(|url| !url.is_empty())
                .collect()
        } else {
            // Fallback to single RPC URL
            vec![CONFIG.rpc_url.clone()]
        };

        info!("Initialized RPC manager with {} URLs", rpc_urls.len());

        Self {
            rpc_urls,
            current_index: Arc::new(RwLock::new(0)),
        }
    }

    /// Get the current RPC client
    pub async fn get_client(&self) -> Arc<RpcClient> {
        let index = *self.current_index.read().await;
        let url = &self.rpc_urls[index];
        Arc::new(RpcClient::new(url.clone()))
    }

    /// Rotate to the next RPC URL and return the new client
    pub async fn rotate_and_get_client(&self) -> Arc<RpcClient> {
        let mut index = self.current_index.write().await;
        let old_index = *index;
        *index = (*index + 1) % self.rpc_urls.len();

        warn!(
            "Rotating RPC client from URL {} to URL {} (index {} -> {})",
            self.rpc_urls[old_index], self.rpc_urls[*index], old_index, *index
        );

        let url = &self.rpc_urls[*index];
        Arc::new(RpcClient::new(url.clone()))
    }

    /// Execute a function with RPC client, with automatic retry on time limit errors
    pub async fn execute_with_retry<F, Fut, T>(&self, operation: F) -> Result<T>
    where
        F: Fn(Arc<RpcClient>) -> Fut + Send + Sync,
        Fut: std::future::Future<Output = Result<T>> + Send,
        T: Send,
    {
        let max_retries = self.rpc_urls.len();
        let mut last_error = None;

        for attempt in 0..max_retries {
            let client = if attempt == 0 {
                self.get_client().await
            } else {
                self.rotate_and_get_client().await
            };

            match operation(client).await {
                Ok(result) => return Ok(result),
                Err(err) => {
                    if self.is_time_limit_error(&err) {
                        warn!(
                            "Time limit error on attempt {} of {}: {}",
                            attempt + 1,
                            max_retries,
                            err
                        );
                        last_error = Some(err);
                        continue;
                    } else {
                        // For non-time-limit errors, return immediately
                        return Err(err);
                    }
                }
            }
        }

        // All retries exhausted
        error!("All {} RPC URLs failed with time limit errors", max_retries);
        Err(last_error.unwrap_or_else(|| {
            ApiError::Custom("All RPC clients failed with time limit errors".to_string())
        }))
    }

    /// Check if the error is a time limit related error
    fn is_time_limit_error(&self, error: &ApiError) -> bool {
        let error_str = error.to_string().to_lowercase();
        error_str.contains("time limit")
            || error_str.contains("timeout")
            || error_str.contains("rate limit")
            || error_str.contains("too many requests")
            || error_str.contains("429")
    }

    /// Get the number of configured RPC URLs
    #[allow(dead_code)]
    pub fn url_count(&self) -> usize {
        self.rpc_urls.len()
    }

    /// Get the current URL index for debugging
    #[allow(dead_code)]
    pub async fn current_url_index(&self) -> usize {
        *self.current_index.read().await
    }
}

impl Default for RpcManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Global RPC manager instance
static RPC_MANAGER: once_cell::sync::Lazy<RpcManager> = once_cell::sync::Lazy::new(RpcManager::new);

/// Get the global RPC manager instance
pub fn get_rpc_manager() -> &'static RpcManager {
    &RPC_MANAGER
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::ApiError;

    #[tokio::test]
    async fn test_rpc_manager_initialization() {
        let manager = RpcManager::new();
        assert_eq!(manager.url_count(), 1); // Should fall back to single URL from CONFIG
        assert_eq!(manager.current_url_index().await, 0);
    }

    #[tokio::test]
    async fn test_rotation() {
        let manager = RpcManager {
            rpc_urls: vec![
                "https://api.mainnet-beta.solana.com".to_string(),
                "https://solana-api.projectserum.com".to_string(),
                "https://api.mainnet-beta.solana.com".to_string(),
            ],
            current_index: Arc::new(RwLock::new(0)),
        };

        assert_eq!(manager.current_url_index().await, 0);

        // Rotate to next
        let _client = manager.rotate_and_get_client().await;
        assert_eq!(manager.current_url_index().await, 1);

        // Rotate again
        let _client = manager.rotate_and_get_client().await;
        assert_eq!(manager.current_url_index().await, 2);

        // Should wrap around
        let _client = manager.rotate_and_get_client().await;
        assert_eq!(manager.current_url_index().await, 0);
    }

    #[test]
    fn test_time_limit_error_detection() {
        let manager = RpcManager::new();

        // Test various error types that should trigger rotation
        let time_limit_errors = vec![
            ApiError::Custom("time limit exceeded".to_string()),
            ApiError::Custom("Request timeout".to_string()),
            ApiError::Custom("Rate limit exceeded".to_string()),
            ApiError::Custom("Too many requests".to_string()),
            ApiError::Custom("HTTP 429 error".to_string()),
        ];

        for error in time_limit_errors {
            assert!(
                manager.is_time_limit_error(&error),
                "Should detect time limit error: {}",
                error
            );
        }

        // Test errors that should NOT trigger rotation
        let other_errors = vec![
            ApiError::Custom("Invalid program ID".to_string()),
            ApiError::Custom("Account not found".to_string()),
            ApiError::Custom("Network error".to_string()),
        ];

        for error in other_errors {
            assert!(
                !manager.is_time_limit_error(&error),
                "Should NOT detect as time limit error: {}",
                error
            );
        }
    }
}
