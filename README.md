# Solana Verified Programs API

This is a hosted wrapper over [solana-verifiable-build](https://github.com/Ellipsis-Labs/solana-verifiable-build/).

## API Endpoints

### Verification

#### Asynchronous Verification
Submit a program for asynchronous verification:

```bash
curl -X POST https://verify.osec.io/verify \
  -H "Content-Type: application/json" \
  -d '{
    "repository": "https://github.com/your-org/your-program",
    "program_id": "PhoeNiXZ8ByJGLkxNfZRnkUfjvmuYqLR89jjFHGqdXY",
    "commit_hash": "abc123...",
    "lib_name": "your_program"
  }'
```

#### Synchronous Verification (Legacy)
To verify a program, simply  add `--remote` to your verification arguments:

```bash
solana-verify verify-from-repo --remote -um --program-id PhoeNiXZ8ByJGLkxNfZRnkUfjvmuYqLR89jjFHGqdXY https://github.com/Ellipsis-Labs/phoenix-v1
```

#### Verification with Specific Signer
Submit verification with a specific signer:

```bash
curl -X POST https://verify.osec.io/verify-with-signer \
  -H "Content-Type: application/json" \
  -d '{
    "signer": "9VWiUUhgNoRwTH5NVehYJEDwcotwYX3VgW4MChiHPAqU",
    "program_id": "PhoeNiXZ8ByJGLkxNfZRnkUfjvmuYqLR89jjFHGqdXY"
  }'
```

### Status Checking

#### Program Verification Status
Check the verification status of a specific program:

```bash
curl https://verify.osec.io/status/PhoeNiXZ8ByJGLkxNfZRnkUfjvmuYqLR89jjFHGqdXY | jq
```

Response format:
```json
{
  "is_verified": true,
  "message": "On chain program verified",
  "on_chain_hash": "5bdb733d10c170fbe08912d258bca0bd15dc52ae4919b7db162f44fa0608516b",
  "executable_hash": "5bdb733d10c170fbe08912d258bca0bd15dc52ae4919b7db162f44fa0608516b",
  "last_verified_at": "2024-02-06T11:36:03.547955",
  "repo_url": "https://github.com/Squads-Protocol/v4/commit/3742e5521a3e833f24a4c6bc024dd1aa5385d010"
}
```

#### Job Status
Check the status of an asynchronous verification job:

```bash
curl https://verify.osec.io/job/f65e0e70-d258-4091-94c1-c039de3734fc | jq
```

#### Build Logs
Retrieve build logs for a program verification:

```bash
curl https://verify.osec.io/logs/PhoeNiXZ8ByJGLkxNfZRnkUfjvmuYqLR89jjFHGqdXY | jq
```

### Verified Programs Lists

#### Get All Verified Programs
```bash
curl https://verify.osec.io/verified-programs | jq
```

#### Get Verified Programs (Paginated)
```bash
curl https://verify.osec.io/verified-programs/1 | jq
```

#### Get Verified Programs Status
```bash
curl https://verify.osec.io/verified-programs-status | jq
```

### Rate Limits

- **Verification endpoints**: 5 requests/second globally, 1 request per 30 seconds per IP
- **Status/query endpoints**: 10,000 requests/second globally, 100 requests/second per IP
- **Unverify endpoint**: 100 requests/second globally

### Security

To mitigate against false verification results, we rerun program verification every 24 hours. Note that verification should not be considered a strict security boundary.

## Otter Verify PDA Worker

The Otter Verify PDA Worker is a service that monitors and processes Program Derived Address (PDA) updates and creations from the Otter Verify program. It automatically updates the database with new PDAs and initiates verification jobs if needed.

### Setup Requirements

1. **Helius Webhook Configuration**
   - Set up a enhanced Helius webhook to monitor all transactions (Select Any in the transaction type filter).
   - Configure the webhook to listen to the Otter Verify program address: `verifycLy8mB96wd9wqq3WDXQwM4oU6r42Th37Db9fC`
   - The webhook should forward transactions to: `https://verify.osec.io/pda`

## Monitor Program upgrades

Monitor program upgrades, and unverify the program if it is upgraded.

### Setup

1. **Helius Webhook Configuration**
   - Set up a enhanced Helius webhook to monitor transactions of type `UPGRADE_PROGRAM_INSTRUCTION`.
   - Configure the webhook to listen to the BPF Loader program address: `BPFLoaderUpgradeab1e11111111111111111111111`
   - The webhook should forward transactions to: `https://verify.osec.io/unverify`

### Security

To ensure that only legitimate requests from our Helius webhook are processed, we add a secret key (defined in `.env`) as a authentication header in the webhook requests.

## Deployment

```bash
cp .env.example .env
# update values in .env
docker compose up --build
```

## Contact

For responsible disclosure of security issues or any other questions, please reach out to <contact@osec.io>
