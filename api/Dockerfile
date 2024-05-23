FROM --platform=linux/amd64 rust:1.78 as api_build
WORKDIR /solana_verified_program_api

# Copy the entire project directory to the Docker image
COPY api ./api
COPY .env .

WORKDIR /solana_verified_program_api/api
RUN cargo build --release

FROM --platform=linux/amd64 rust:1.78 as api_final
WORKDIR /solana_verified_program_api

RUN cargo install solana-verify --git https://github.com/Ellipsis-Labs/solana-verifiable-build --rev 098551fd107212b9e86293d18c608cbb94f5d446
COPY --from=api_build /solana_verified_program_api/api/target/release/verified_programs_api .
RUN apt-get update && apt-get install -y docker.io

CMD ["./verified_programs_api"]