FROM rust:1.95-slim AS build
WORKDIR /src
COPY . .
RUN cargo build --release

FROM debian:trixie-slim
COPY --from=build /src/target/release/infino-gateway /src/target/release/infino-worker /usr/local/bin/
