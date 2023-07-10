use crate::diesel::{ExpressionMethods, RunQueryDsl};
use crate::schema;
use diesel::r2d2::PooledConnection;
use diesel::result::Error as DieselError;
use diesel::QueryDsl;

use crate::errors::ApiError;
use diesel::{
    r2d2::{ConnectionManager, Pool},
    PgConnection,
};
use std::process::Command;
use std::sync::Arc;

use crate::models::{SolanaProgramBuild, SolanaProgramBuildParams, VerfiedProgram};

pub async fn verify_build(
    pool: Arc<Pool<ConnectionManager<PgConnection>>>,
    payload: SolanaProgramBuildParams,
) -> Result<VerfiedProgram, ApiError> {
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
                let verified_build = VerfiedProgram {
                    id: uuid::Uuid::new_v4().to_string(),
                    program_id: payload.program_id.clone(),
                    is_verified: last_line.contains("Program hash matches"),
                    on_chain_hash: onchain_hash,
                    executable_hash: build_hash,
                    verified_at: chrono::Utc::now().naive_utc(),
                };
                let _ = insert_verified_build(&verified_build, pool).await;
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

fn get_last_line(output: &str) -> Option<String> {
    output.lines().last().map(|line| line.to_owned())
}

fn extract_hash(output: &str, prefix: &str) -> Option<String> {
    if let Some(line) = output.lines().find(|line| line.starts_with(prefix)) {
        let hash = line.trim_start_matches(prefix.trim()).trim();
        Some(hash.to_owned())
    } else {
        None
    }
}

// DB operations
pub async fn get_db_connection(
    pool: Arc<Pool<ConnectionManager<PgConnection>>>,
) -> Result<PooledConnection<ConnectionManager<PgConnection>>, diesel::result::Error> {
    let conn = pool.get();

    let conn = match conn {
        Ok(conn) => conn,
        Err(err) => {
            tracing::error!("Failed to get connection: {}", err);
            return Err(DieselError::DatabaseError(
                diesel::result::DatabaseErrorKind::ClosedConnection,
                Box::new(err.to_string()),
            ));
        }
    };
    Ok(conn)
}

pub async fn insert_build(
    payload: &SolanaProgramBuild,
    pool: Arc<Pool<ConnectionManager<PgConnection>>>,
) -> Result<(), diesel::result::Error> {
    let conn = &mut get_db_connection(pool).await?;

    diesel::insert_into(schema::solana_program_builds::table)
        .values(payload)
        .on_conflict(schema::solana_program_builds::program_id)
        .do_update()
        .set(payload)
        .execute(conn)?;

    Ok(())
}

pub async fn insert_verified_build(
    payload: &VerfiedProgram,
    pool: Arc<Pool<ConnectionManager<PgConnection>>>,
) -> Result<(), diesel::result::Error> {
    let conn = &mut get_db_connection(pool).await?;

    diesel::insert_into(schema::verified_programs::table)
        .values(payload)
        .on_conflict(schema::verified_programs::program_id)
        .do_update()
        .set(payload)
        .execute(conn)?;

    Ok(())
}

pub async fn get_build(
    program_address: String,
    conn: &mut PooledConnection<ConnectionManager<PgConnection>>,
) -> Result<SolanaProgramBuild, diesel::result::Error> {
    let res = schema::solana_program_builds::table
        .filter(schema::solana_program_builds::program_id.eq(program_address))
        .first::<SolanaProgramBuild>(conn)?;

    Ok(res)
}

pub async fn check_is_program_verified(
    program_address: String,
    pool: Arc<Pool<ConnectionManager<PgConnection>>>,
) -> Result<bool, diesel::result::Error> {
    let conn = &mut get_db_connection(pool.clone()).await?;

    let res = schema::verified_programs::table
        .filter(schema::verified_programs::program_id.eq(&program_address))
        .first::<VerfiedProgram>(conn);

    match res {
        Ok(res) => {
            // check if the program is verified less than 24 hours ago
            let now = chrono::Utc::now().naive_utc();
            let verified_at = res.verified_at;
            let diff = now - verified_at;
            if diff.num_hours() > 24 {
                // if the program is verified more than 24 hours ago, rebuild and verify
                let payload = get_build(program_address, conn).await?;
                tokio::spawn(async move {
                    let _ = verify_build(
                        pool,
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
