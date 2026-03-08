FROM rust:1.82-bookworm AS builder

WORKDIR /app

# Copy workspace
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/

# Build release binary (Base only — no Solana)
ENV CARGO_BUILD_JOBS=2
RUN cargo build --release -p atr-server

# Runtime image
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/atr-server /usr/local/bin/atr-server

# Create data directory for SQLite
RUN mkdir -p /data
WORKDIR /data

ENV DATABASE_URL=sqlite:/data/atr.db?mode=rwc
ENV PORT=3000

EXPOSE 3000

CMD ["atr-server"]
