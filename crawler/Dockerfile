FROM --platform=linux/amd64 rust:1.74 as crawler_build
WORKDIR /solana_verified_program_api

COPY crawler ./crawler
COPY security-txt ./security-txt
COPY schema.rs .
COPY diesel.toml .
COPY .env .

WORKDIR /solana_verified_program_api/crawler
RUN cargo build --release

FROM --platform=linux/amd64 rust:1.74 as crawler_final
WORKDIR /solana_verified_program_api

COPY --from=crawler_build /solana_verified_program_api/crawler/target/release/crawler .
RUN apt-get update && apt-get install -y cron
COPY ./crawler/script.sh ./script.sh
RUN chmod +x ./script.sh

ENTRYPOINT ["./script.sh"]
