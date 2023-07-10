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
      "message": "Build verification started",
      "success": true
  }
  ```

  - If there are errors the response will be:

  ```json
  {
      "message": "Error message",
      "success": false
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
      "message": "On chain program verified",
      "success": true
  }
  ```

  - else the response will be:

  ```json
  {
      "message": "On chain program not verified",
      "success": false
  }
  ```
