# syntax=docker/dockerfile:1
FROM rust:1.95-slim AS build
RUN apt-get update \
    && apt-get install -y --no-install-recommends git ca-certificates \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /src

# The engine commit to build with (see build-versions.sh).
ARG ENGINE_REV
COPY get-engine.sh .
RUN ./get-engine.sh "$ENGINE_REV" engine

COPY . .
# Cache downloads and build output, so the second version only rebuilds the engine.
# Both versions use the same engine path, so touch its files: otherwise cargo can
# keep the engine compiled for the other version.
RUN --mount=type=cache,target=/usr/local/cargo/registry \
    --mount=type=cache,target=/src/target \
    find engine -type f -exec touch {} + \
    && cargo build --release \
    && cp target/release/infino-gateway target/release/infino-worker /usr/local/bin/

FROM debian:trixie-slim
COPY --from=build /usr/local/bin/infino-gateway /usr/local/bin/infino-worker /usr/local/bin/
