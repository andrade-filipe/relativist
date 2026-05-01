# Stage 1: Build (builder image — debian-based rust:slim matches bookworm)
FROM rust:slim AS builder
WORKDIR /app

# --- Dependency caching layer ---
# Copy workspace manifests first so Docker can cache the `cargo build` step
# independently from source changes (the dummy-source trick). Only the Cargo
# manifests are present here; both crates get a minimal stub so Cargo can
# compile their dependency trees.
COPY Cargo.toml Cargo.lock ./
COPY relativist-core/Cargo.toml relativist-core/Cargo.toml
COPY relativist-cli/Cargo.toml relativist-cli/Cargo.toml

RUN mkdir -p relativist-core/src relativist-cli/src && \
    echo "" > relativist-core/src/lib.rs && \
    echo "fn main() {}" > relativist-cli/src/main.rs && \
    cargo build --release -p relativist-cli --locked && \
    rm -rf relativist-core/src relativist-cli/src

# --- Real source ---
# These COPY layers invalidate independently:
# - changing relativist-core/ invalidates both layers and the full rebuild.
# - changing relativist-cli/ only invalidates this second layer; the
#   relativist-core/ dep-cache layer (from the stub build) stays CACHED.
COPY relativist-core/ relativist-core/
COPY relativist-cli/ relativist-cli/

RUN cargo build --release -p relativist-cli --locked

# Stage 2: Runtime (match glibc with the builder's Debian base)
# rust:slim uses Debian bookworm at Rust 1.84+; runtime must match.
FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/relativist /usr/local/bin/relativist

WORKDIR /data

ENTRYPOINT ["/usr/local/bin/relativist"]
