# Build stage
FROM rust:1.83-bookworm AS builder

WORKDIR /app

RUN apt-get update && apt-get install -y pkg-config libssl-dev && rm -rf /var/lib/apt/lists/*

# Copy workspace files
COPY Cargo.toml Cargo.lock ./

# Copy only the crates needed for server (not CLI)
COPY dvaar_common ./dvaar_common
COPY dvaar_server ./dvaar_server

# Create dummy CLI crate to satisfy workspace (won't be built)
RUN mkdir -p dvaar_cli/src && echo "fn main() {}" > dvaar_cli/src/main.rs
COPY dvaar_cli/Cargo.toml ./dvaar_cli/

# Build only the server binary
RUN cargo build --release --bin dvaar_server

# Runtime stage
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates libssl3 curl && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy only server binary and migrations
COPY --from=builder /app/target/release/dvaar_server /usr/local/bin/
COPY --from=builder /app/dvaar_server/migrations ./migrations

# Run as non-root user
RUN useradd -m -s /bin/bash dvaar
USER dvaar

EXPOSE 8080 6000

CMD ["dvaar_server"]
