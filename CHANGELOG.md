# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [1.5.1] - 2026-02-17

### Added

- **Webhook callback support** for async verify APIs
- **Search query parameter** for the verified programs endpoint
- **Redis health check** in the `/health` endpoint
- **Validation** for verify endpoints: pubkeys and URL format

### Changed

- **Dependencies:** general dependency updates
- **Solana-related crates:** updated to current versions (e.g. solana-sdk 4.0.0, solana-client 3.1.8, solana-transaction-status 3.1.8, solana-sdk-ids 3.1.0)
