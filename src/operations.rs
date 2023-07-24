use crate::errors::ApiError;
use crate::state::AppState;
use std::process::Command;

use crate::models::{SolanaProgramBuildParams, VerifiedProgram};
use crate::utils::{extract_hash, get_last_line};

/// The `verify_build` function verifies a Solana program build by executing the `solana-verify` command
/// and parsing the output to determine if the program hash matches and storing the verified build
/// information in a database.
///
/// Arguments:
///
/// * `pool`: `pool` is an Arc of a connection pool to a PostgreSQL database. It is used to interact
/// with the database and perform database operations.
/// * `payload`: The `payload` parameter is of type `SolanaProgramBuildParams`
///
/// Returns:
///
/// The function `verify_build` returns a `Result` with the success case containing a `VerifiedProgram`
/// struct and the error case containing an `ApiError`.
pub async fn verify_build(
    app_state: AppState,
    payload: SolanaProgramBuildParams,
) -> Result<VerifiedProgram, ApiError> {
    tracing::info!("Verifying build..");
    let mut cmd = Command::new("solana-verify");
    cmd.arg("verify-from-repo")
        .arg("-um")
        .arg("--program-id")
        .arg(&payload.program_id)
        .arg(payload.repository);

    if let Some(commit) = payload.commit_hash {
        cmd.arg("--commit-hash").arg(commit);
    }

    if let Some(library_name) = payload.lib_name {
        cmd.arg("--library-name").arg(library_name);
    }

    if let Some(bpf_flag) = payload.bpf_flag {
        if bpf_flag {
            cmd.arg("--bpf");
        }
    }

    let output = cmd.output();

    if let Ok(output) = output {
        if output.status.success() {
            let result = String::from_utf8(output.stdout);
            let result = match result {
                Ok(result) => result,
                Err(err) => {
                    tracing::error!("Failed to get the output from program: {}", err);
                    return Err(ApiError::ParseError(
                        "Failed to get the output from program".to_owned(),
                    ));
                }
            };

            let onchain_hash = extract_hash(&result, "On-chain Program Hash:").unwrap_or_default();
            let build_hash =
                extract_hash(&result, "Executable Program Hash from repo:").unwrap_or_default();

            // last line of output has the result
            if let Some(last_line) = get_last_line(&result) {
                let verified_build = VerifiedProgram {
                    id: uuid::Uuid::new_v4().to_string(),
                    program_id: payload.program_id.clone(),
                    is_verified: last_line.contains("Program hash matches"),
                    on_chain_hash: onchain_hash,
                    executable_hash: build_hash,
                    verified_at: chrono::Utc::now().naive_utc(),
                };
                let _ = app_state
                    .db_client
                    .insert_or_update_verified_build(&verified_build)
                    .await;
                Ok(verified_build)
            } else {
                tracing::error!("Failed to get the output from program.");
                Err(ApiError::Custom(
                    "Failed to get the output from program".to_owned(),
                ))
            }
        } else {
            tracing::error!("Failed to execute the program.");
            Err(ApiError::BuildError)
        }
    } else {
        tracing::error!("Failed to execute the program.");
        Err(ApiError::BuildError)
    }
}

/// The function `check_is_program_verified_within_24hrs` checks if a program is verified within the last 24 hours
/// and rebuilds and verifies it if it is not.
///
/// Arguments:
///
/// * `program_address`: The `program_address` parameter is a string that represents the address of a
/// program. It is used to query the database and check if the program is verified.
///
/// Returns: Whether the program is verified or not. The return type is
/// a `Result<bool, diesel::result::Error>`.
pub async fn check_is_program_verified_within_24hrs(
    app_state: AppState,
    program_address: String,
) -> Result<bool, diesel::result::Error> {
    let res = app_state
        .db_client
        .get_verified_build(&program_address)
        .await;

    match res {
        Ok(res) => {
            // check if the program is verified less than 24 hours ago
            let now = chrono::Utc::now().naive_utc();
            let verified_at = res.verified_at;
            let diff = now - verified_at;
            if diff.num_hours() > 24 {
                // if the program is verified more than 24 hours ago, rebuild and verify
                let payload = app_state
                    .db_client
                    .get_build_params(&program_address)
                    .await?;
                tokio::spawn(async move {
                    let _ = verify_build(
                        app_state,
                        SolanaProgramBuildParams {
                            repository: payload.repository,
                            program_id: payload.program_id,
                            commit_hash: payload.commit_hash,
                            lib_name: payload.lib_name,
                            bpf_flag: Some(payload.bpf_flag),
                        },
                    )
                    .await;
                });
            }
            Ok(res.is_verified)
        }
        Err(err) => {
            if err.to_string() == "Record not found" {
                return Ok(false);
            }
            Err(err)
        }
    }
}

pub async fn check_is_build_params_exists_already(
    app_state: AppState,
    payload: &SolanaProgramBuildParams,
) -> Result<bool, diesel::result::Error> {
    let build = app_state
        .db_client
        .get_build_params(&payload.program_id)
        .await?;

    if build.repository == payload.repository
        && build.commit_hash == payload.commit_hash
        && build.lib_name == payload.lib_name
        && build.bpf_flag == payload.bpf_flag.unwrap_or(false)
    {
        return Ok(true);
    }

    Ok(false)
}
