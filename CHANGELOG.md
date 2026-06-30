# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **`GET /resolve-hash/{executable_hash}` endpoint**: content-addressed lookup over the verified-build catalogue. Given a 64-char executable hash, returns every completed build that produced it, each flagged with `matches_deployed` (true when the hash matches the program's currently-deployed on-chain hash).


## [1.5.5] - 2026-06-29

### Added

- **Workspace path support** for monorepo verification: requests can now include `workspace_path`, build records persist it, and verification passes it to `solana-verify --workspace-path`.

## [1.5.4] - 2026-06-23

### Fixed

- GitHub links in `/status`, `/status-all` no longer include `.git` before `/tree/{commit}`, which was causing 404s for some repos.


## [1.5.3] - 2026-04-06

### Added

- **CI workflow** (`.github/workflows/ci.yaml`): runs `cargo fmt`, `cargo clippy`, `cargo sort`, and `cargo machete` on every push and PR to `master`.
- **`rust-toolchain.toml`**: pins Rust toolchain to `1.93` (matching the Dockerfile), shared between local dev and CI.

### Changed

- **`solana-verify` version:** updated to `v0.4.15` in `api/Dockerfile`.
- **`solana-verify` source repo links:** updated from `Ellipsis-Labs/solana-verifiable-build` to `solana-foundation/solana-verifiable-build`.

### Fixed

- **Re-verification always marked `is_verified=false`**: fixed per-row `is_verified` computation when on-chain hash changes, preventing builds with matching hashes from being incorrectly unverified.
- **Duplicate phantom build record on every verification**: removed the spurious `initial_uuid` row that was inserted and immediately marked completed before the real verification build started.

### Removed

- **`use-external-pdas` feature flag**: dead code — the feature gated imports that were never used anywhere in the codebase.

## [1.5.2] - 2026-03-25

### Added

- **Webhook callback support** for async verify APIs
- **Search query parameter** for the verified programs endpoint
- **Optional `error` field** in the verified programs list response
- **Redis health check** in the `/health` endpoint
- **Validation** for verify endpoints: pubkeys and URL format

### Changed

- **Root endpoint (`/`)**: now serves a simple HTML landing page for verify.osec.io (intro, contact, link to solana-verifiable-build docs). API endpoint list is at `GET /api`.
- **Dependencies:** general dependency updates
- **Solana-related crates:** updated to current versions (e.g. solana-sdk 4.0.0, solana-client 3.1.8, solana-transaction-status 3.1.8, solana-sdk-ids 3.1.0)
- **Refactoring**: Extract shared input validation helpers for `/verify`, `/verify-with-signer`, and `/verify_sync` to remove duplicated logic.
