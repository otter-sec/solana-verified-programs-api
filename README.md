# Solana Verified Programs API

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
    "status": "success",
    "is_verified": true,
    "on_chain_hash": "72da599d9ee14b2a03a23ccfa6f06d53eea4a00825ad2191929cbd78fb69205c",
    "executable_hash": "72da599d9ee14b2a03a23ccfa6f06d53eea4a00825ad2191929cbd78fb69205c",
    "message": "On chain program verified"
  }
  ```

  - Incase if the hashes doesn't match

  ```json
  {
    "status": "success",
    "is_verified": false,
    "on_chain_hash": "72da599d9ee14b2a03a23ccfa6f06d53eea4a00825ad2191929cbd78fb69205c",
    "executable_hash": "fed5d956fc389b5b09a354340d479b07cfc66dd9f8d0af76a0e8e950c1c58680",
    "message": "On chain program not verified"
  }
  ```

  - If there are any errors the response will be:

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
    "is_verified": true,
    "message": "On chain program verified"
  }
  ```

  - else the response will be:

  ```json
  {
    "is_verified": false,
    "message": "On chain program is not verified"
  }
  ```
