# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [2.0.4] - 2026-07-22

### Fixed

- **On-chain snapshot WARNs**: stop logging full program ELF bytes; keep account type, slot, and authority only.

## [2.0.3] - 2026-07-21

### Changed

- **`GET /status-all/{address}` now returns verified builds first**: preserves one selected build per signer while ordering hash-matching entries before unverified entries.

## [2.0.2] - 2026-07-14

### Changed

- **Per-IP rate limit on status/list endpoints**: increased to ~100 req/s with a burst of 200 (was burst 100, then ~1/s refill). Stops Explorer CI from 429ing while paging `/verified-programs`.

## [2.0.1] - 2026-07-13

### Fixed

- **`GET /status-all/{address}` could report `is_verified: false` while `/status` was true**: now picks each signer's hash-matching build instead of the newest completed row ([#139](https://github.com/otter-sec/solana-verified-programs-api/pull/139)).

## [2.0.0] - 2026-07-06

### Added

- **`GET /resolve-hash/{executable_hash}` endpoint**: content-addressed lookup over the verified-build catalogue. Given a 64-char executable hash, returns every completed build that produced it, each flagged with `matches_deployed` (true when the hash matches the program's currently-deployed on-chain hash).
- **Integration test suite** (`tests/`): end-to-end coverage of route behaviour, the v1→v2 migration, the background sweep, and webhook callbacks, plus a `verify-smoke` workflow and `workflow_dispatch` trigger on CI.

### Changed

- **v2 rewrite of the API service**, replacing the v1 stack:
  - Diesel + Redis → sqlx + an in-process cache
  - `verified_programs` / `solana_program_builds` / `program_authority` tables → `builds` + `program_state`
  - per-program `solana-verify` hash subprocess → in-process hashing over batched `getMultipleAccounts`
  - hourly status job → drift-driven sweep with automatic re-verification
- **Webhook endpoints (`/pda`, `/unverify`) are no longer rate-limited** — now gated only by the `AUTHORIZATION` header (previously `/unverify` was capped at 100 req/s).
- **`solana-verify` version:** updated to `v0.5.1` in `install-verify.sh`.

### Fixed

- **Router middleware no longer leaks across route groups**: rate-limit and CORS layers applied to routes beyond their group (e.g. a GET CORS policy on the POST `/verify*` routes). Each group is now a separate merged router.


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
