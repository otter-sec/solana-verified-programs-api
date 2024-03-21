use anyhow::anyhow;
use crossbeam_channel::{unbounded, Receiver};
use reqwest::Client;
use serde_json::json;
use std::thread;
use std::time::Duration;

use crate::api::models::{
    ErrorResponse, JobResponse, JobStatus, JobVerificationResponse, VerifyResponse,
};

use super::models::SolanaProgramBuildParams;

// URL for the remote server
pub const REMOTE_SERVER_URL: &str = "https://verify.osec.io";

fn poll_and_wait_for_result(receiver: Receiver<bool>) {
    loop {
        match receiver.try_recv() {
            Ok(result) => {
                if result {
                    tracing::info!("✅ Request processing completed successfully.");
                } else {
                    tracing::error!("❌ Request processing failed.");
                }
                break;
            }

            Err(_) => {
                thread::sleep(Duration::from_millis(100));
            }
        }
    }
}

// Send a job to the remote server
pub async fn verify_build(params: SolanaProgramBuildParams) -> anyhow::Result<()> {
    let client = Client::builder()
        .timeout(Duration::from_secs(18000))
        .build()?;

    // Send the POST request
    let response = client
        .post(format!("{}/verify", REMOTE_SERVER_URL))
        .json(&json!({
            "repository": params.repository,
            "commit_hash": params.commit_hash,
            "program_id": params.program_id,
            "lib_name": params.lib_name,
            "bpf_flag": params.bpf_flag,
            "mount_path":  if params.mount_path.is_none() {
                None
            } else {
                Some(params.mount_path)
            },
            "base_image": params.base_image,
            "cargo_args": params.cargo_args,
        }))
        .send()
        .await?;

    if response.status().is_success() {
        let status_response: VerifyResponse = response.json().await?;
        tracing::info!("Verification request sent. ✅");
        tracing::info!("Verification in progress... ⏳");
        // Span new thread for polling the server for status
        // Create a channel for communication between threads
        let (sender, receiver) = unbounded();

        let handle = thread::spawn(move || poll_and_wait_for_result(receiver));
        // Poll the server for status
        loop {
            let status = check_job_status(&client, &status_response.request_id).await?;
            match status.status {
                JobStatus::InProgress => {
                    thread::sleep(Duration::from_secs(10));
                }
                JobStatus::Completed => {
                    let _ = sender.send(true);
                    handle.join().unwrap();
                    let status_response = status.respose.unwrap();
                    tracing::info!(
                        "Program {} has been successfully verified. ✅",
                        params.program_id
                    );
                    tracing::info!("\nThe provided GitHub build matches the on-chain hash:");
                    tracing::info!("On Chain Hash: {}", status_response.on_chain_hash.as_str());
                    tracing::info!(
                        "Executable Hash: {}",
                        status_response.executable_hash.as_str()
                    );
                    tracing::info!("Repo URL: {}", status_response.repo_url.as_str());
                    break;
                }
                JobStatus::Failed => {
                    let _ = sender.send(false);

                    handle.join().unwrap();
                    let status_response: JobVerificationResponse = status.respose.unwrap();
                    tracing::error!("Program {} has not been verified. ❌", params.program_id);
                    tracing::error!("Error message: {}", status_response.message.as_str());
                    break;
                }
                JobStatus::Unknown => {
                    let _ = sender.send(false);
                    handle.join().unwrap();
                    tracing::warn!("Program {} has not been verified. ❌", params.program_id);
                    break;
                }
            }
        }

        Ok(())
    } else if response.status() == 409 {
        let response = response.json::<ErrorResponse>().await?;
        tracing::error!("Error: {}", response.error.as_str());
        Ok(())
    } else {
        tracing::error!("Encountered an error while attempting to send the job to remote");
        Err(anyhow!("{:?}", response.text().await?))?
    }
}

async fn check_job_status(client: &Client, request_id: &str) -> anyhow::Result<JobResponse> {
    // Get /job/:id
    let response = client
        .get(&format!("{}/job/{}", REMOTE_SERVER_URL, request_id))
        .send()
        .await
        .unwrap();

    if response.status().is_success() {
        // Parse the response
        let response: JobVerificationResponse = response.json().await?;
        match response.status {
            JobStatus::InProgress => {
                thread::sleep(Duration::from_secs(5));
                Ok(JobResponse {
                    status: JobStatus::InProgress,
                    respose: None,
                })
            }
            JobStatus::Completed => Ok(JobResponse {
                status: JobStatus::Completed,
                respose: Some(response),
            }),
            JobStatus::Failed => Ok(JobResponse {
                status: JobStatus::Failed,
                respose: Some(response),
            }),
            JobStatus::Unknown => Ok(JobResponse {
                status: JobStatus::Unknown,
                respose: Some(response),
            }),
        }
    } else {
        Err(anyhow!(
            "Encountered an error while attempting to check job status : {:?}",
            response.text().await?
        ))?
    }
}
