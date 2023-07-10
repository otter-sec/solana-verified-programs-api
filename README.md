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

```bash
curl --location 'localhost:3000/verify' \
--header 'Content-Type: application/json' \
--data '{
  "repository": "https://github.com/Squads-Protocol/squads-mpl",
  "commit_hash": "c95b7673d616c377a349ca424261872dfcf8b19d",
  "program_id": "SMPLecH534NA9acpos4G6x7uf3LWbCAwZQE9e8ZekMu",
  "lib_name": "squads_mpl",
  "bpf_flag": true
}'
```

- Upon submitting a job the endpint will start a new verification of the program and returns a message.

  - If there are no errors the response will be:

  ```json
  {
    "type": "VerifyAsync",
    "status": "success",
    "message": "Build verification started"
  }
  ```

  - If there are errors the response will be:

  ```json
  {
    "status":"error",
    "error":"Error message here"
  }
  ```

### Synchronously verify a program

```bash
curl --location 'localhost:3000/verify_sync' \
--header 'Content-Type: application/json' \
--data '{
  "repository": "https://github.com/Ellipsis-Labs/phoenix-v1",
  "program_id": "PhoeNiXZ8ByJGLkxNfZRnkUfjvmuYqLR89jjFHGqdXY"
}'
```

```bash
curl --location 'localhost:3000/verify_sync' \
--header 'Content-Type: application/json' \
--data '{
  "repository": "https://github.com/Squads-Protocol/squads-mpl",
  "commit_hash": "c95b7673d616c377a349ca424261872dfcf8b19d",
  "program_id": "SMPLecH534NA9acpos4G6x7uf3LWbCAwZQE9e8ZekMu",
  "lib_name": "squads_mpl",
  "bpf_flag": true
}'
```

- Upon submitting a job the endpint will start a new verification of the program and returns status once it finishes the job.

  - When the job is finished the response will be:

  ```json
    {
      "executable_hash": "7c76ba11f8742d040b1a874d943c2096f1b3a48db14d2a5b411fd5dad5d1bc2d",
      "message": "Build verification completed",
      "on_chain_hash": "7c76ba11f8742d040b1a874d943c2096f1b3a48db14d2a5b411fd5dad5d1bc2d",
      "success": true
    }
  ```

  - Incase if the hashes doesn't match

  ```json
    {
      "executable_hash": "7c76ba11f8742d040b1a874d943c2096f1b3a48db14d2a5b411fd5dad5d1bc2d",
      "message": "Build verification completed",
      "on_chain_hash": "G13ab11f8742d040b1a874d943c2096f1b3a48db14d2a5b411fd5dad5a1ec3e1",
      "success": false
    }
  ```

  - If there are asny errors the response will be:

  ```json
  {
    "status":"error",
    "error":"Error message here"
  }
  ```

### Get the status of a verification

```bash
curl --location 'localhost:3000/status/PhoeNiXZ8ByJGLkxNfZRnkUfjvmuYqLR89jjFHGqdXY'
```

- Returns the status of the verification

  - If the given program is verified the response will be:

  ```json
  {
    "type": "VerificationStatus",
    "is_verified": true,
    "message": "On chain program verified"
  }
  ```

  - else the response will be:

  ```json
  {
    "type": "VerificationStatus",
    "is_verified": false,
    "message": "On chain program is not verified"
  }
  ```
