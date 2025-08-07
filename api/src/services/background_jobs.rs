use crate::{db::DbClient, services::rpc_manager::get_rpc_manager, Result, CONFIG};
use futures::stream::{self, StreamExt};
use std::{sync::Arc, time::Duration};
use tokio::time;
use tracing::{error, info, warn};

/// Background job manager for periodic tasks
pub struct BackgroundJobManager {
    db_client: DbClient,
}

impl BackgroundJobManager {
    /// Create a new background job manager
    pub fn new(db_client: DbClient) -> Self {
        Self { db_client }
    }

    /// Get background job health status
    pub async fn get_health_status(&self) -> BackgroundJobHealth {
        // Check when the last program check was performed by looking at database timestamps
        match self.get_last_program_check_timestamp().await {
            Ok(last_program_check) => {
                let now = chrono::Utc::now().naive_utc();
                let time_since_check = now - last_program_check;
                let expected_interval =
                    chrono::Duration::seconds(CONFIG.program_status_update_interval_seconds as i64);

                if time_since_check > expected_interval * 2 {
                    BackgroundJobHealth {
                        status: "Inactive".to_string(),
                        last_program_check: Some(last_program_check),
                        message: format!(
                            "Last program check was {} seconds ago, expected interval is {} seconds",
                            time_since_check.num_seconds(),
                            CONFIG.program_status_update_interval_seconds
                        ),
                    }
                } else {
                    BackgroundJobHealth {
                        status: "Active".to_string(),
                        last_program_check: Some(last_program_check),
                        message: "Background jobs are running normally".to_string(),
                    }
                }
            }
            Err(_) => BackgroundJobHealth {
                status: "unknown".to_string(),
                last_program_check: None,
                message: "Unable to determine when programs were last checked".to_string(),
            },
        }
    }

    async fn get_last_program_check_timestamp(&self) -> Result<chrono::NaiveDateTime> {
        // Try to get the last job execution time from cache/storage
        if let Ok(last_execution) = self.get_last_job_execution_time().await {
            return Ok(last_execution);
        }

        // Fallback: use the most recent program authority update as an approximation
        let conn = &mut self.db_client.get_db_conn().await?;

        let query = r#"
            SELECT MAX(last_updated) as last_update
            FROM program_authority
            WHERE last_updated IS NOT NULL
        "#;

        use diesel::{sql_query, sql_types::Nullable, sql_types::Timestamp, QueryableByName};
        use diesel_async::RunQueryDsl;

        #[derive(QueryableByName)]
        struct LastUpdateResult {
            #[diesel(sql_type = Nullable<Timestamp>)]
            last_update: Option<chrono::NaiveDateTime>,
        }

        let result = sql_query(query)
            .get_result::<LastUpdateResult>(conn)
            .await?;

        result.last_update.ok_or_else(|| {
            crate::errors::ApiError::Custom("No program check timestamps found".to_string())
        })
    }

    /// Store the timestamp when the background job last executed
    async fn store_job_execution_time(&self, execution_time: chrono::NaiveDateTime) -> Result<()> {
        let cache_key = "background_job:last_execution";
        let timestamp_str = execution_time.format("%Y-%m-%d %H:%M:%S%.3f").to_string();
        self.db_client.set_cache(cache_key, &timestamp_str).await?;
        Ok(())
    }

    /// Get the timestamp when the background job last executed
    async fn get_last_job_execution_time(&self) -> Result<chrono::NaiveDateTime> {
        let cache_key = "background_job:last_execution";
        let timestamp_str = self.db_client.get_cache(cache_key).await?;

        chrono::NaiveDateTime::parse_from_str(&timestamp_str, "%Y-%m-%d %H:%M:%S%.3f")
            .map_err(|e| crate::errors::ApiError::Custom(format!("Failed to parse timestamp: {e}")))
    }

    /// Start all background jobs
    pub async fn start_all_jobs(&self) {
        info!("Starting background job manager");

        // Start program status update job
        let db_client = self.db_client.clone();
        tokio::spawn(async move {
            program_status_update_job(db_client).await;
        });

        // Start health monitoring job
        let db_client_health = self.db_client.clone();
        tokio::spawn(async move {
            health_monitoring_job(db_client_health).await;
        });

        info!("All background jobs started successfully");
    }
}

/// Health monitoring job that periodically logs background job status
async fn health_monitoring_job(db_client: DbClient) {
    // Run health checks every 30 minutes
    let mut interval = time::interval(Duration::from_secs(1800));
    let bg_manager = BackgroundJobManager::new(db_client);

    info!("Health monitoring job started with 30-minute intervals");

    loop {
        interval.tick().await;

        let health_status = bg_manager.get_health_status().await;
        match health_status.status.as_str() {
            "healthy" => info!("Background jobs health check: {}", health_status.message),
            "unhealthy" => warn!(
                "Background jobs health check UNHEALTHY: {}",
                health_status.message
            ),
            _ => warn!(
                "Background jobs health check UNKNOWN: {}",
                health_status.message
            ),
        }
    }
}

/// Background job that periodically updates program status (frozen/closed)
async fn program_status_update_job(db_client: DbClient) {
    let mut interval = time::interval(Duration::from_secs(
        CONFIG.program_status_update_interval_seconds,
    ));
    let mut consecutive_errors = 0u32;
    const MAX_CONSECUTIVE_ERRORS: u32 = 5;

    info!(
        "Program status update job started with interval: {} seconds",
        CONFIG.program_status_update_interval_seconds
    );

    let bg_manager = BackgroundJobManager::new(db_client.clone());

    loop {
        interval.tick().await;

        info!("Starting program status update cycle");
        let start_time = std::time::Instant::now();
        let execution_time = chrono::Utc::now().naive_utc();

        // Store the execution timestamp
        if let Err(e) = bg_manager.store_job_execution_time(execution_time).await {
            warn!("Failed to store job execution time: {:?}", e);
        }

        match update_all_program_status(&db_client).await {
            Ok(updated_count) => {
                let duration = start_time.elapsed();
                info!(
                    "Program status update completed: {} programs updated in {:?}",
                    updated_count, duration
                );
                consecutive_errors = 0; // Reset error counter on success
            }
            Err(e) => {
                consecutive_errors += 1;
                error!(
                    "Program status update failed (attempt {}/{}): {:?}",
                    consecutive_errors, MAX_CONSECUTIVE_ERRORS, e
                );

                if consecutive_errors >= MAX_CONSECUTIVE_ERRORS {
                    error!(
                        "Too many consecutive failures ({}), extending interval to avoid system overload", 
                        consecutive_errors
                    );
                    // Wait longer before next attempt to avoid overwhelming the system
                    tokio::time::sleep(Duration::from_secs(300)).await; // 5 minutes
                    consecutive_errors = 0; // Reset after extended wait
                }
            }
        }
    }
}

/// Update status for all verified programs
async fn update_all_program_status(db_client: &DbClient) -> Result<usize> {
    // Get all verified program IDs from database
    let all_programs = db_client.get_all_verified_program_ids().await?;
    let total_programs = all_programs.len();

    info!("Found {} verified programs to check", total_programs);

    if all_programs.is_empty() {
        return Ok(0);
    }

    let _rpc_manager = get_rpc_manager();
    let batch_size = CONFIG.program_status_batch_size;
    let max_concurrent = CONFIG.program_status_max_concurrent;
    let semaphore = Arc::new(tokio::sync::Semaphore::new(max_concurrent));

    let mut updated_count = 0;
    let mut batch_number = 0;

    // Process programs in batches
    for batch in all_programs.chunks(batch_size) {
        batch_number += 1;
        info!(
            "Processing batch {}: {} programs",
            batch_number,
            batch.len()
        );

        let batch_updates: Vec<ProgramStatusUpdate> = stream::iter(batch.to_vec())
            .map(|program_id| {
                let semaphore = Arc::clone(&semaphore);

                async move {
                    let _permit = semaphore
                        .acquire()
                        .await
                        .expect("Semaphore should not be closed");

                    check_program_status(&program_id).await
                }
            })
            .buffer_unordered(max_concurrent)
            .filter_map(|result| async move { result.ok() })
            .collect()
            .await;

        // Update database with batch results
        if !batch_updates.is_empty() {
            match update_program_status_batch(db_client, batch_updates).await {
                Ok(batch_updated) => {
                    updated_count += batch_updated;
                    info!(
                        "Batch {} completed: {} programs updated",
                        batch_number, batch_updated
                    );
                }
                Err(e) => {
                    error!("Failed to update batch {}: {:?}", batch_number, e);
                }
            }
        }

        // Small delay between batches to avoid overwhelming the system
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    info!(
        "Status update completed: {}/{} programs updated",
        updated_count, total_programs
    );
    Ok(updated_count)
}

/// Check status of a single program
async fn check_program_status(program_id: &str) -> Result<ProgramStatusUpdate> {
    use crate::services::onchain::get_program_authority;

    match get_program_authority(program_id).await {
        Ok((authority, is_frozen, is_closed)) => Ok(ProgramStatusUpdate {
            program_id: program_id.to_string(),
            authority,
            is_frozen,
            is_closed,
            last_checked: chrono::Utc::now().naive_utc(),
        }),
        Err(e) => {
            warn!("Failed to check status for program {}: {:?}", program_id, e);
            // For programs we can't check, assume they might be closed
            Ok(ProgramStatusUpdate {
                program_id: program_id.to_string(),
                authority: None,
                is_frozen: false,
                is_closed: true, // Assume closed if we can't fetch authority
                last_checked: chrono::Utc::now().naive_utc(),
            })
        }
    }
}

/// Update program status in database for a batch of programs
async fn update_program_status_batch(
    db_client: &DbClient,
    updates: Vec<ProgramStatusUpdate>,
) -> Result<usize> {
    let mut updated_count = 0;

    for update in updates {
        match update_single_program_status(db_client, &update).await {
            Ok(true) => updated_count += 1,
            Ok(false) => {} // No update needed
            Err(e) => {
                error!(
                    "Failed to update status for program {}: {:?}",
                    update.program_id, e
                );
            }
        }
    }

    Ok(updated_count)
}

/// Update status for a single program, returns true if update was made
async fn update_single_program_status(
    db_client: &DbClient,
    update: &ProgramStatusUpdate,
) -> Result<bool> {
    use solana_sdk::pubkey::Pubkey;
    use std::str::FromStr;

    let program_pubkey = Pubkey::from_str(&update.program_id)?;

    // Check current status in database
    let current_frozen = db_client
        .is_program_frozen(&update.program_id)
        .await
        .unwrap_or(false);
    let current_closed = db_client
        .is_program_closed(&update.program_id)
        .await
        .unwrap_or(false);

    // Only update if status has changed
    if current_frozen != update.is_frozen || current_closed != update.is_closed {
        info!(
            "Program {} status changed - frozen: {} -> {}, closed: {} -> {} (checked at {})",
            update.program_id,
            current_frozen,
            update.is_frozen,
            current_closed,
            update.is_closed,
            update.last_checked
        );

        db_client
            .insert_or_update_program_authority(
                &program_pubkey,
                update.authority.as_deref(),
                update.is_frozen,
                Some(update.is_closed),
            )
            .await?;

        return Ok(true);
    }

    Ok(false)
}

/// Represents a program status update
#[derive(Debug, Clone)]
struct ProgramStatusUpdate {
    program_id: String,
    authority: Option<String>,
    is_frozen: bool,
    is_closed: bool,
    last_checked: chrono::NaiveDateTime,
}

/// Background job health status
#[derive(Debug, Clone, serde::Serialize)]
pub struct BackgroundJobHealth {
    pub status: String,
    pub last_program_check: Option<chrono::NaiveDateTime>,
    pub message: String,
}

impl DbClient {
    /// Get all verified program IDs for status checking
    pub async fn get_all_verified_program_ids(&self) -> Result<Vec<String>> {
        let conn = &mut self.get_db_conn().await?;

        let query = r#"
            SELECT DISTINCT program_id
            FROM verified_programs
            WHERE is_verified = true
            ORDER BY program_id
        "#;

        use diesel::{sql_query, sql_types::Text, QueryableByName};
        use diesel_async::RunQueryDsl;

        #[derive(QueryableByName)]
        struct ProgramIdResult {
            #[diesel(sql_type = Text)]
            program_id: String,
        }

        let program_ids: Vec<String> = sql_query(query)
            .get_results::<ProgramIdResult>(conn)
            .await
            .map_err(|e| {
                error!("Failed to fetch all verified program IDs: {}", e);
                e
            })?
            .into_iter()
            .map(|result| result.program_id)
            .collect();

        info!(
            "Retrieved {} verified program IDs for status checking",
            program_ids.len()
        );
        Ok(program_ids)
    }
}
