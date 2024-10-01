// src/api/index.rs

use axum::Json;
use serde_json::{json, Value};
use std::sync::OnceLock;

static INDEX_JSON: OnceLock<Value> = OnceLock::new();

pub fn index() -> Json<Value> {
    let value = INDEX_JSON.get_or_init(|| {
        json!({
            "endpoints": [
                {
                    "path": "/verify",
                    "method": "POST",
                    "description": "Verify a program",
                    "params" : {
                        "repo": "Git repository URL",
                        "program_id": "Program ID of the program in mainnet",
                        "commit": "(Optional) Commit hash of the repository. If not specified, the latest commit will be used.",
                        "lib_name": "(Optional) If the repository contains multiple programs, specify the name of the library name of the program to build and verify.",
                        "bpf_flag": "(Optional)  If the program requires cargo build-bpf (instead of cargo build-sbf), as for an Anchor program, set this flag.",
                        "base_image": "(Optional) Base docker image to use for building the program.",
                        "mount_path": "(Optional) Mount path for the repository.",
                        "cargo_args": "(Optional) Cargo args to pass to the build command. It should be Vector of strings."
                    },
                },
                {
                    "path": "/status/:address",
                    "method": "GET",
                    "description": "Check the verification status of a program by its address",
                    "params": {
                        "address": "Address of the mainnet program to check the verification status"
                    }
                },
                {
                    "path": "/verified-programs",
                    "method": "GET",
                    "description": "Get the list of verified programs"
                }
            ]
        })
    });
    Json(value.clone())
}
