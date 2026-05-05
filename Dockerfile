# Stage 1: Build — pin builder to Debian bookworm so its GLIBC matches the
# runtime stage. `rust:slim` (untagged) tracks the latest Debian and now
# points to Trixie (GLIBC 2.39), which mismatches `debian:bookworm-slim`
# (GLIBC 2.36) and breaks the runtime image with a dynamic linker error.
FROM rust:slim-bookworm AS builder
WORKDIR /app

# --- Dependency caching layer ---
# Copy workspace manifests first so Docker can cache the `cargo build` step
# independently from source changes (the dummy-source trick). Only the Cargo
# manifests are present here; both crates get a minimal stub so Cargo can
# compile their dependency trees.
COPY Cargo.toml Cargo.lock ./
COPY relativist-core/Cargo.toml relativist-core/Cargo.toml
COPY relativist-cli/Cargo.toml relativist-cli/Cargo.toml

RUN mkdir -p relativist-core/src relativist-core/benches relativist-cli/src && \
    echo "" > relativist-core/src/lib.rs && \
    echo "fn main() {}" > relativist-core/benches/benchmarks.rs && \
    echo "fn main() {}" > relativist-cli/src/main.rs && \
    cargo build --release -p relativist-cli --locked && \
    rm -rf relativist-core/src relativist-core/benches relativist-cli/src \
           target/release/relativist \
           target/release/deps/relativist* \
           target/release/.fingerprint/relativist-cli-* \
           target/release/.fingerprint/relativist-core-*

# --- Real source ---
# These COPY layers invalidate independently:
# - changing relativist-core/ invalidates both layers and the full rebuild.
# - changing relativist-cli/ only invalidates this second layer; the
#   relativist-core/ dep-cache layer (from the stub build) stays CACHED.
#
# IMPORTANT: the stub-binary, fingerprints and `deps/relativist*` are wiped at
# the end of the cache layer above. Without that, BuildKit COPY preserves
# (deterministic) mtimes that can be older than the stub fingerprint, so cargo
# silently keeps the stub binary instead of rebuilding from real source.
COPY relativist-core/ relativist-core/
COPY relativist-cli/ relativist-cli/

RUN cargo build --release -p relativist-cli --locked

# Stage 2: Runtime — must match the builder's Debian base (GLIBC alignment).
# Builder is pinned to `rust:slim-bookworm`, so this stays bookworm-slim.
FROM debian:bookworm-slim

RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/relativist /usr/local/bin/relativist

WORKDIR /data

ENTRYPOINT ["/usr/local/bin/relativist"]
