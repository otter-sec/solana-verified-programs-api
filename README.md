# Solana Verified Programs APP

## Usage

```bash
docker compose up --build
```

## How to use API

### Start a new verification of a program

```bash
curl --location 'localhost:3000/verify' \
--header 'Content-Type: application/json' \
--data '{
  "repository": "https://github.com/Ellipsis-Labs/phoenix-v1",
  "program_id": "PhoeNiXZ8ByJGLkxNfZRnkUfjvmuYqLR89jjFHGqdXY"
}'
```

### Get the status of a verification

```bash
curl --location 'localhost:3000/status/PhoeNiXZ8ByJGLkxNfZRnkUfjvmuYqLR89jjFHGqdXY'
```
