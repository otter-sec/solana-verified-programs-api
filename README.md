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
  "on_chain_hash": "6877a5b732b3494b828a324ec846d526d962223959534dbaf4209e0da3b2d6a9",
  "executable_hash": "6877a5b732b3494b828a324ec846d526d962223959534dbaf4209e0da3b2d6a9"
}
```

## Deployment

```bash
docker-compose up --build
```
