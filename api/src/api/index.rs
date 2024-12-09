// src/api/index.rs

use axum::Json;
use serde_json::{json, Value};
use std::sync::OnceLock;

/// Static JSON response for the index endpoint
static INDEX_JSON: OnceLock<Value> = OnceLock::new();

/// Handler for the index endpoint that provides API documentation
///
/// # Endpoint: GET /
///
/// # Returns
/// * `Json<Value>` - JSON response containing API endpoint documentation
pub fn index() -> Json<Value> {
    let value = INDEX_JSON.get_or_init(|| {
        json!({
            "endpoints": [
                {
                    "path": "/verify",
                    "method": "POST",
                    "description": "Asynchronously verify a Solana program",
                    "params": {
                        "repository": {
                            "type": "string",
                            "required": true,
                            "description": "Git repository URL containing the program source code"
                        },
                        "program_id": {
                            "type": "string",
                            "required": true,
                            "description": "Solana program ID on mainnet"
                        },
                        "commit_hash": {
                            "type": "string",
                            "required": true,
                            "description": "Specific Git commit hash to verify. Defaults to latest commit"
                        },
                        "lib_name": {
                            "type": "string",
                            "required": false,
                            "description": "Library name for repositories with multiple programs"
                        },
                        "bpf_flag": {
                            "type": "boolean",
                            "required": false,
                            "description": "Use cargo build-bpf instead of cargo build-sbf (required for Anchor programs)"
                        },
                        "base_image": {
                            "type": "string",
                            "required": false,
                            "description": "Custom Docker base image for building"
                        },
                        "mount_path": {
                            "type": "string",
                            "required": false,
                            "description": "Custom mount path for repository in build container"
                        },
                        "cargo_args": {
                            "type": "array",
                            "items": "string",
                            "required": false,
                            "description": "Additional cargo build arguments"
                        }
                    }
                },
                {
                    "path": "/verify/sync",
                    "method": "POST",
                    "description": "Synchronously verify a Solana program",
                    "params": {
                        "$ref": "#/endpoints/0/params"
                    }
                },
                {
                    "path": "/status/:address",
                    "method": "GET",
                    "description": "Check program verification status",
                    "params": {
                        "address": {
                            "type": "string",
                            "required": true,
                            "description": "Mainnet program address to check"
                        }
                    }
                },
                {
                    "path": "/verified-programs",
                    "method": "GET",
                    "description": "Get list of all verified programs",
                    "params": {}
                },
                {
                    "path": "/verified-programs/status",
                    "method": "GET",
                    "description": "Get detailed status of all verified programs",
                    "params": {}
                },
                {
                    "path": "/jobs/:job_id",
                    "method": "GET",
                    "description": "Check status of an async verification job",
                    "params": {
                        "job_id": {
                            "type": "string",
                            "required": true,
                            "description": "Verification job identifier"
                        }
                    }
                },
                {
                    "path": "/logs/:address",
                    "method": "GET",
                    "description": "Get build logs for a program",
                    "params": {
                        "address": {
                            "type": "string",
                            "required": true,
                            "description": "Program address to fetch logs for"
                        }
                    }
                }
            ]
        })
    });

    Json(value.clone())
}
