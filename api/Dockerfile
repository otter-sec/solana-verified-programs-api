FROM --platform=linux/amd64 rust:1.84 AS api_build

WORKDIR /solana_verified_program_api

# Copy the entire project directory to the Docker image
COPY api ./api

COPY .env .

WORKDIR /solana_verified_program_api/api

RUN cargo build --release

RUN cargo install solana-verify --git https://github.com/Ellipsis-Labs/solana-verifiable-build

FROM --platform=linux/amd64 rust:1.84 AS api_final

WORKDIR /solana_verified_program_api

COPY --from=api_build /solana_verified_program_api/api/target/release/verified_programs_api .

COPY --from=api_build /usr/local/cargo/bin/solana-verify /usr/local/bin/solana-verify

RUN apt-get update && apt-get install -y docker.io libpq-dev

CMD ["./verified_programs_api"]
