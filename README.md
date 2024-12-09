# Solana Verified Programs API

This is a hosted wrapper over [solana-verifiable-build](https://github.com/Ellipsis-Labs/solana-verifiable-build/).

## Verification

To verify a program, simply add `--remote` to your verification arguments.

```bash
solana-verify verify-from-repo --remote -um --program-id PhoeNiXZ8ByJGLkxNfZRnkUfjvmuYqLR89jjFHGqdXY https://github.com/Ellipsis-Labs/phoenix-v1
```

## Status

The `/status` endpoint is designed to be used to check the status of a verification job. To mitigate against false verification results, we rerun program verification every 24 hours. Note that regardless, verification should not be considered a strict security boundary.

```bash
$ curl --location 'https://verify.osec.io/status/PhoeNiXZ8ByJGLkxNfZRnkUfjvmuYqLR89jjFHGqdXY' | jq
{
  "is_verified": true,
  "message": "On chain program verified",
  "on_chain_hash": "5bdb733d10c170fbe08912d258bca0bd15dc52ae4919b7db162f44fa0608516b",
  "executable_hash": "5bdb733d10c170fbe08912d258bca0bd15dc52ae4919b7db162f44fa0608516b",
  "last_verified_at": "2024-02-06T11:36:03.547955",
  "repo_url": "https://github.com/Squads-Protocol/v4/commit/3742e5521a3e833f24a4c6bc024dd1aa5385d010"
}
```

## Deployment

```bash
docker-compose up --build
```

## Contact

For responsible disclosure of security issues or any other questions, please reach out to <contact@osec.io>
