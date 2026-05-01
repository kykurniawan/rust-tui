FROM rust:1.86-slim-bookworm AS builder

WORKDIR /app

# Copy workspace manifests
COPY Cargo.toml Cargo.lock ./
COPY rustui-server/Cargo.toml rustui-server/Cargo.toml
COPY rustui-management/Cargo.toml rustui-management/Cargo.toml
COPY rustui-client/Cargo.toml rustui-client/Cargo.toml

# Create dummy source files to build dependencies only (layer caching)
RUN mkdir -p rustui-server/src rustui-management/src rustui-client/src && \
    echo 'fn main() {}' > rustui-server/src/main.rs && \
    echo 'fn main() {}' > rustui-management/src/main.rs && \
    echo 'fn main() {}' > rustui-client/src/main.rs && \
    echo 'pub mod crypto;' > rustui-client/src/lib.rs

# Build dependencies (will be cached)
RUN cargo build --release -p rustui-server -p rustui-management && \
    rm -rf rustui-server/src rustui-management/src rustui-client/src

# Copy actual source
COPY rustui-server/src rustui-server/src
COPY rustui-management/src rustui-management/src
COPY rustui-client/src rustui-client/src

# Build actual binaries (touch to invalidate cached dummy build)
RUN touch rustui-server/src/main.rs rustui-management/src/main.rs && \
    cargo build --release -p rustui-server -p rustui-management

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates && \
    rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/rustui-server /usr/local/bin/rustui-server
COPY --from=builder /app/target/release/rustui-management /usr/local/bin/rustui-management

# Create data directory (also auto-created at runtime)
RUN mkdir -p /root/.rustui

EXPOSE 8080

CMD ["rustui-server"]
