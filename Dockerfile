FROM rust:1.85-bookworm AS builder

WORKDIR /app

# Copy workspace
COPY Cargo.toml Cargo.lock ./
COPY crates/ crates/

# Build with minimal memory: 1 job, minimal codegen units
ENV CARGO_BUILD_JOBS=1
ENV RUSTFLAGS="-C codegen-units=1"
RUN cargo build --release -p atr-server --bin atr-server

# Runtime image
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/atr-server /usr/local/bin/atr-server

RUN mkdir -p /data
WORKDIR /data

ENV DATABASE_URL=sqlite:/data/atr.db?mode=rwc
ENV PORT=3000

EXPOSE 3000

CMD ["atr-server"]
