# Stage 1: Build (rust:slim uses Debian trixie; runtime must match)
FROM rust:slim AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY src/ src/
COPY benches/ benches/
RUN cargo build --release

# Stage 2: Runtime (must match builder's glibc version — Debian trixie)
FROM debian:trixie-slim
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/relativist /usr/local/bin/relativist
ENTRYPOINT ["relativist"]
