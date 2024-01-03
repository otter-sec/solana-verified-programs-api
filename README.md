# Solana Verified Programs API

This is a hosted wrapper over [solana-verifiable-build](https://github.com/Ellipsis-Labs/solana-verifiable-build/).

## API

### Usage

- We recommend that developers use `/verify` to verify programs. This endpoint is designed to be used asynchronously. It will return a response immediately, and the verification process will run in the background. This endpoint is designed to be used by developers to submit verification jobs without waiting for the results.
- The `/status` endpoint is designed to be used by explorers to check the status of a verification job. This endpoint is designed to be used by explorers to check the status of a verification job.

### Using Ellipsis Labs CLI

To install the Solana Verify cli, run the following in your shell:

```bash
# Pulls the latest version from crates.io
cargo install solana-verify
```

To verify a program, run the following in your shell:

```bash
solana-verify verify-from-repo -um --program-id PhoeNiXZ8ByJGLkxNfZRnkUfjvmuYqLR89jjFHGqdXY https://github.com/Ellipsis-Labs/phoenix-v1
```

Final Output:

```bash
Executable Program Hash from repo: 7c76ba11f8742d040b1a874d943c2096f1b3a48db14d2a5b411fd5dad5d1bc2d
On-chain Program Hash: 7c76ba11f8742d040b1a874d943c2096f1b3a48db14d2a5b411fd5dad5d1bc2d
Program hash matches ✅
```

### Verification

Note that the parameters are equivalent to what is used on the Phoenix CLI.

```bash
curl --location 'https://verify.osec.io/verify' \
--header 'Content-Type: application/json' \
--data '{
  "repository": "https://github.com/Ellipsis-Labs/phoenix-v1",
  "program_id": "PhoeNiXZ8ByJGLkxNfZRnkUfjvmuYqLR89jjFHGqdXY"
}'
```

```bash
curl --location 'https://verify.osec.io/verify' \
--header 'Content-Type: application/json' \
--data '{
  "repository": "https://github.com/Squads-Protocol/squads-mpl",
  "commit_hash": "c95b7673d616c377a349ca424261872dfcf8b19d", # optional
  "program_id": "SMPLecH534NA9acpos4G6x7uf3LWbCAwZQE9e8ZekMu",
  "lib_name": "squads_mpl",
  "bpf_flag": true
}'
```

Upon submitting a job the endpoint will start a new verification of the program and returns the following:

```json
{
  "status": "success" // or "error",
  "message": "Build verification started" // or an error message
}
```

If the request was duplicate we will return the following response:

```json
{
    "is_verified": true, // or `false` if hashes don't match
    "message": "On chain program verified", // or an error message
    "on_chain_hash": "72da599d9ee14b2a03a23ccfa6f06d53eea4a00825ad2191929cbd78fb69205c", // only returned on success
    "executable_hash": "72da599d9ee14b2a03a23ccfa6f06d53eea4a00825ad2191929cbd78fb69205c", // only returned on success
    "repo_url": "https://github.com/Squads-Protocol/squads-mpl/commit/c95b7673d616c377a349ca424261872dfcf8b19d" // only returned on success
}
```

### Synchronous Verification

```bash
curl --location 'https://verify.osec.io/verify_sync' \
--header 'Content-Type: application/json' \
--data '{
  "repository": "https://github.com/Squads-Protocol/squads-mpl",
  "commit_hash": "c95b7673d616c377a349ca424261872dfcf8b19d",
  "program_id": "SMPLecH534NA9acpos4G6x7uf3LWbCAwZQE9e8ZekMu",
  "lib_name": "squads_mpl",
  "bpf_flag": true
}'
```

Upon submitting a job the endpoint will start a new verification of the program. The response will be:

```json
{
  "status": "success", // or "error"
  "is_verified": true, // or `false` if hashes don't match
  "on_chain_hash": "72da599d9ee14b2a03a23ccfa6f06d53eea4a00825ad2191929cbd78fb69205c", // only returned on success
  "executable_hash": "72da599d9ee14b2a03a23ccfa6f06d53eea4a00825ad2191929cbd78fb69205c", // only returned on success
  "message": "On-chain program verified" // or an error message
}
```

### Get the status of a verification

```bash
curl --location 'https://verify.osec.io/status/PhoeNiXZ8ByJGLkxNfZRnkUfjvmuYqLR89jjFHGqdXY'
```

The response will be:

```json
{
  "is_verified": true, // or `false` if hashes don't match
  "message": "On chain program verified" // or an error message
}
```

### Cache

- The program verification cache is configured for a duration of 24 hours. After this period, we compare the on-chain hash, and if it doesn't match our local hash, the verification process is rerun. This ensures that the program remains verified on-chain.

## Deployment

```bash
docker compose up --build
```
