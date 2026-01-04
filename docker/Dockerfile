# Build stage
FROM rust:1.83-bookworm AS builder

WORKDIR /app

RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

COPY Cargo.toml Cargo.lock ./
COPY dvaar_common ./dvaar_common
COPY dvaar_server ./dvaar_server
COPY dvaar_cli ./dvaar_cli

RUN cargo build --release --bin dvaar_server --bin dvaar

# Runtime stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates libssl3 curl && rm -rf /var/lib/apt/lists/*

WORKDIR /app

COPY --from=builder /app/target/release/dvaar_server /usr/local/bin/
COPY --from=builder /app/target/release/dvaar /usr/local/bin/
COPY --from=builder /app/dvaar_server/migrations ./migrations

RUN useradd -m -s /bin/bash dvaar
USER dvaar

EXPOSE 8080 6000

CMD ["dvaar_server"]
