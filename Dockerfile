FROM --platform=linux/amd64 rust:1.70 as build

RUN USER=root cargo new --bin verified_programs_api
WORKDIR /verified_programs_api

COPY ./Cargo.lock ./Cargo.lock
COPY ./Cargo.toml ./Cargo.toml

RUN cargo build --release
RUN rm src/*.rs

COPY ./src ./src

RUN rm ./target/release/deps/verified_programs_api*
RUN cargo build --release


FROM --platform=linux/amd64 rust:1.70
RUN cargo install solana-verify
COPY --from=build /verified_programs_api/target/release/verified_programs_api .
RUN apt-get update && apt-get install -y docker.io
CMD ["./verified_programs_api"]