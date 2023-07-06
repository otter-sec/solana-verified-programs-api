use crate::diesel::{ExpressionMethods, RunQueryDsl};
use crate::schema;
use diesel::QueryDsl;

use diesel::{
    r2d2::{ConnectionManager, Pool},
    PgConnection,
};
use std::process::Command;

use crate::models::{SolanaProgramBuild, SolanaProgramBuildParams, VerfiedProgram};

pub async fn verify_build(
    pool: Pool<ConnectionManager<PgConnection>>,
    payload: SolanaProgramBuildParams,
) {
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

    let output = cmd.output().expect("Failed to execute command");

    if output.status.success() {
        let result = String::from_utf8(output.stdout).unwrap();

        // last line of output is the result
        if let Some(last_line) = get_last_line(&result) {
            if last_line.contains("Program hash matches") {
                println!("Program hashes match");
                let verified_build = VerfiedProgram {
                    id: uuid::Uuid::new_v4().to_string(),
                    program_id: payload.program_id.clone(),
                    is_verified: true,
                    verified_at: chrono::Utc::now().naive_utc(),
                };
                let _ = insert_verified_build(&verified_build, pool).await;
            } else {
                println!("Program hashes do not match");
            }
        } else {
            println!("Failed to execute the program.");
        }
    } else {
        let result = String::from_utf8(output.stderr).unwrap();
        println!("Result: {}", result);
    }
}

fn get_last_line(output: &str) -> Option<String> {
    output.lines().last().map(|line| line.to_owned())
}

pub async fn insert_build(
    payload: &SolanaProgramBuild,
    pool: Pool<ConnectionManager<PgConnection>>,
) -> Result<(), diesel::result::Error> {
    let conn = &mut pool.get().unwrap();

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
    pool: Pool<ConnectionManager<PgConnection>>,
) -> Result<(), diesel::result::Error> {
    let conn = &mut pool.get().unwrap();

    diesel::insert_into(schema::verified_programs::table)
        .values(payload)
        .on_conflict(schema::verified_programs::program_id)
        .do_update()
        .set(payload)
        .execute(conn)?;

    Ok(())
}

pub async fn check_is_program_verified(
    program_address: String,
    pool: Pool<ConnectionManager<PgConnection>>,
) -> Result<bool, diesel::result::Error> {
    let conn = &mut pool.get().unwrap();

    let res = schema::verified_programs::table
        .filter(schema::verified_programs::program_id.eq(program_address))
        .first::<VerfiedProgram>(conn);

    match res {
        Ok(res) => Ok(res.is_verified),
        Err(err) => {
            if err.to_string() == "Record not found" {
                return Ok(false);
            }
            Err(err)
        }
    }
}
