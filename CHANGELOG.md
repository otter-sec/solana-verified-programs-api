# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.5.3] - 2026-04-01

### Changed

- **Docker image:** install `solana-verify` from [solana-foundation/solana-verifiable-build](https://github.com/solana-foundation/solana-verifiable-build) v0.4.15 (previously Ellipsis-Labs v0.4.12).

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
