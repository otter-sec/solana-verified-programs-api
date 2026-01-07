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
                    "path": "/",
                    "method": "GET",
                    "description": "API endpoint documentation",
                    "params": {}
                },
                {
                    "path": "/verify",
                    "method": "POST",
                    "description": "Deprecated: use /verify-with-signer. Asynchronously verify a Solana program",
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
                        },
                        "arch": {
                            "type": "string",
                            "required": false,
                            "description": "Build for the given target architecture [default: v0]"
                        }
                    }
                },
                {
                    "path": "/verify-with-signer",
                    "method": "POST",
                    "description": "Preferred endpoint. Asynchronously verify using PDA params for the provided signer, PDA signer should be the program authority",
                    "params": {
                        "signer": {
                            "type": "string",
                            "required": true,
                            "description": "PDA signer public key should be the program authority"
                        },
                        "program_id": {
                            "type": "string",
                            "required": true,
                            "description": "Solana program ID on mainnet"
                        }
                    }
                },
                {
                    "path": "/verify_sync",
                    "method": "POST",
                    "description": "Deprecated: use /verify-with-signer. Synchronously verify a Solana program",
                    "params": {
                        "$ref": "#/endpoints/1/params"
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
                    "path": "/status-all/:address",
                    "method": "GET",
                    "description": "Get all verification information for a program",
                    "params": {
                        "address": {
                            "type": "string",
                            "required": true,
                            "description": "Mainnet program address to check"
                        }
                    }
                },
                {
                    "path": "/job/:job_id",
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
                },
                {
                    "path": "/verified-programs",
                    "method": "GET",
                    "description": "Get list of all verified programs",
                    "params": {}
                },
                {
                    "path": "/verified-programs/:page",
                    "method": "GET",
                    "description": "Get paginated list of verified programs",
                    "params": {
                        "page": {
                            "type": "integer",
                            "required": true,
                            "description": "Page number (starting from 1)"
                        }
                    }
                },
                {
                    "path": "/verified-programs-status",
                    "method": "GET",
                    "description": "Get detailed status of all verified programs",
                    "params": {}
                },
            ]
        })
    });

    Json(value.clone())
}
