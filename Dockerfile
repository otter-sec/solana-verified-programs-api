FROM rust:1.94 AS api_build

WORKDIR /solana_verified_program_api

# Install system dependencies required by hidapi/solana-verify
RUN apt-get update && apt-get install -y \
    libudev-dev \
    pkg-config \
    && rm -rf /var/lib/apt/lists/*

# Copy the entire project directory to the Docker image
COPY . .

RUN cargo build --release

RUN cargo install solana-verify --git https://github.com/solana-foundation/solana-verifiable-build --tag v0.5.0

FROM debian:stable-slim AS api_final

WORKDIR /solana_verified_program_api

COPY --from=api_build /solana_verified_program_api/target/release/verified_programs_api .

COPY --from=api_build /usr/local/cargo/bin/solana-verify /usr/local/bin/solana-verify

RUN apt-get update && apt-get install -y docker.io ca-certificates && rm -rf /var/lib/apt/lists/*

CMD ["./verified_programs_api"]
