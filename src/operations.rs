use crate::diesel::RunQueryDsl;
use crate::schema;

use diesel::{
    r2d2::{ConnectionManager, Pool},
    PgConnection,
};
use std::process::Command;

use crate::models::{SolanaProgramBuild, SolanaProgramBuildParams};

pub async fn verify_build(payload: SolanaProgramBuildParams) {
    let mut cmd = Command::new("solana-verify");
    cmd.arg("verify-from-repo")
        .arg("-um")
        .arg("--program-id")
        .arg(payload.program_id)
        .arg(payload.repository);

    if let Some(commit) = payload.commit_hash {
        cmd.arg("--commit-hash").arg(commit);
    }

    if let Some(library_name) = payload.lib_name {
        cmd.arg("--library-name").arg(library_name);
    }

    if payload.bpf_flag {
        cmd.arg("--bpf");
    }

    let output = cmd.output().expect("Failed to execute command");

    if !output.status.success() {
        println!("Failed to execute command");
        let result = String::from_utf8(output.stderr).unwrap();
        println!("Result: {}", result);
    } else {
        println!("Success");
        let result = String::from_utf8(output.stdout).unwrap();
        println!("Result: {}", result);
    }
}

pub async fn insert_build(
    payload: &SolanaProgramBuild,
    pool: Pool<ConnectionManager<PgConnection>>,
) -> Result<(), diesel::result::Error> {
    let conn = &mut pool.get().unwrap();

    diesel::insert_into(schema::solana_program_builds::table)
        .values(payload)
        .execute(conn)?;

    Ok(())
}
