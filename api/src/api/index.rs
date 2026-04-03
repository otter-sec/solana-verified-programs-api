// src/api/index.rs

use axum::{Json, response::Html};
use serde_json::{Value, json};
use std::sync::OnceLock;

/// Static JSON response for the index endpoint
static INDEX_JSON: OnceLock<Value> = OnceLock::new();

static LANDING_HTML: &str = r#"<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1" />
    <title>Solana Verified Builds</title>
    <style>
      :root { color-scheme: light dark; }
      body { font-family: ui-sans-serif, system-ui, -apple-system, Segoe UI, Roboto, Helvetica, Arial, "Apple Color Emoji", "Segoe UI Emoji"; line-height: 1.5; margin: 0; }
      main { max-width: 880px; margin: 0 auto; padding: 56px 20px; }
      h1 { font-size: 32px; margin: 0 0 12px; }
      p { margin: 0 0 16px; }
      .card { border: 1px solid rgba(127,127,127,.25); border-radius: 12px; padding: 16px; margin: 18px 0; }
      .muted { opacity: .85; }
      a { color: inherit; }
      ul { margin: 10px 0 0 18px; padding: 0; }
      footer { margin-top: 28px; font-size: 14px; opacity: .85; }
      code { font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", "Courier New", monospace; }
    </style>
  </head>
  <body>
    <main>
      <h1>Solana Verifiable Build</h1>
      <p class="muted">
        Verified builds help users confirm that an on-chain Solana program matches its public source code.
      </p>

      <div class="card">
        <strong>Need help?</strong>
        <ul>
          <li>Email: <a href="mailto:contact@osec.io">contact@osec.io</a></li>
          <li>GitHub: <a href="https://github.com/otter-sec/solana-verified-programs-api">otter-sec/solana-verified-programs-api</a></li>
        </ul>
      </div>

      <div class="card">
        <strong>Docs</strong>
        <p class="muted" style="margin-top:10px;">
          Learn how to create verified builds in the official documentation.
        </p>
        <p style="margin:0;">
          <a href="https://solana.com/docs/programs/verified-builds#how-do-i-create-verified-builds">
            Solana docs: Verified Builds (How do I create verified builds?)
          </a>
        </p>
        <p class="muted" style="margin-top:12px;">
          Build tool: <a href="https://github.com/solana-foundation/solana-verifiable-build">solana-verifiable-build</a>
        </p>
      </div>

      <footer>
        Looking for the API? See <code>GET /api</code> for the endpoint list.
      </footer>
    </main>
  </body>
</html>
"#;

/// Simple landing page for https://verify.osec.io
pub fn landing_page() -> Html<&'static str> {
    Html(LANDING_HTML)
}

/// Handler for the index endpoint that provides API documentation
///
/// # Endpoint: GET /api
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
                    "description": "Landing page",
                    "params": {}
                },
                {
                    "path": "/api",
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
                        },
                        "webhook_url": {
                            "type": "string",
                            "required": false,
                            "description": "Webhook URL to receive verification results"
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
                        },
                        "webhook_url": {
                            "type": "string",
                            "required": false,
                            "description": "Webhook URL to receive verification results"
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
                    "path": "/logs/:build_id",
                    "method": "GET",
                    "description": "Build logs for a job",
                    "params": {
                        "build_id": {
                            "type": "string",
                            "required": true,
                            "description": "Job id (UUID)"
                        }
                    }
                },
                {
                    "path": "/verified-programs",
                    "method": "GET",
                    "description": "Get list of all verified programs",
                    "params": {},
                    "query": {
                        "search": {
                            "type": "string",
                            "required": false,
                            "description": "Filter by program_id or repository (must be valid Solana address or HTTP/HTTPS URL)"
                        }
                    }
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
                    },
                    "query": {
                        "search": {
                            "type": "string",
                            "required": false,
                            "description": "Filter by program_id or repository (must be valid Solana address or HTTP/HTTPS URL)"
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
