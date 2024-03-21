// Github Client

use anyhow::Result;
use serde_json::Value;

use crate::errors;

pub struct GithubClient {
    owner: String,
    repo: String,
    client: reqwest::Client,
}
static USER_AGENT: &str = "GitHub-otter-sec";

// impl GithubClient
impl GithubClient {
    pub fn new(owner: &str, repo: &str) -> Self {
        Self {
            owner: owner.to_string(),
            repo: repo.to_string(),
            client: reqwest::Client::new(),
        }
    }

    pub async fn fetch_default_branch(&self) -> Result<String> {
        let url = format!("https://api.github.com/repos/{}/{}", self.owner, self.repo);

        let response = self
            .client
            .get(&url)
            .header(reqwest::header::USER_AGENT, USER_AGENT)
            .header(reqwest::header::ACCEPT, "application/vnd.github+json")
            .send()
            .await?
            .json::<Value>()
            .await?;

        if let Some(default_branch) = response["default_branch"].as_str() {
            Ok(default_branch.to_string())
        } else {
            tracing::error!("Default branch not found");
            Err(errors::CrawlerErrors::DefaultBranchNotFound.into())
        }
    }

    pub async fn get_verification_json(
        &self,
    ) -> Result<serde_json::Map<String, serde_json::Value>> {
        let branch = self.fetch_default_branch().await?;

        // Source code/.verified-build.json
        let url = format!(
            "https://raw.githubusercontent.com//{}/{}/{}/.verified-build.json",
            self.owner, self.repo, branch
        );

        // Create a reqwest client
        let content = self
            .client
            .get(&url)
            .send()
            .await?
            .json::<serde_json::Value>()
            .await
            .map_err(|e| {
                tracing::error!("Failed to fetch JSON: {}", e);
                errors::CrawlerErrors::InvalidJsonFileContents
            })?;

        content
            .as_object()
            .cloned()
            .ok_or_else(|| anyhow::format_err!("Failed to parse JSON due to an invalid format."))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_fetch_default_branch() {
        let client = GithubClient::new("Ellipsis-Labs", "phoenix-v1");
        let result = client.fetch_default_branch().await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "master");
    }

    #[tokio::test]
    async fn test_get_verification_json() {
        let client = GithubClient::new("Ellipsis-Labs", "phoenix-v1");
        let result = client.get_verification_json().await;
        assert!(result.is_ok());
        assert!(result
            .unwrap()
            .contains_key("PhoeNiXZ8ByJGLkxNfZRnkUfjvmuYqLR89jjFHGqdXY"));
    }
}
