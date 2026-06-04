# Build stage
FROM rust:1.89-slim-bookworm AS builder

# Install system dependencies (build-essential, pkg-config, etc. if needed)
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy dependency manifests
COPY Cargo.toml Cargo.lock ./

# Create dummy source file to pre-compile dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release
RUN rm -rf src

# Copy real source code
COPY src ./src

# Build the release binary
# Touch main.rs to force cargo to rebuild it instead of using cached dummy binary
RUN touch src/main.rs
RUN cargo build --release

# Run stage
FROM debian:bookworm-slim

WORKDIR /app

# Install ca-certificates and sqlite3 if needed for runtime
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    sqlite3 \
    ffmpeg \
    && rm -rf /var/lib/apt/lists/*

# Copy compiled binary from builder
COPY --from=builder /app/target/release/audion-server /app/audion-server

# Expose server port
EXPOSE 8080

# Set environment defaults
ENV AUDION_DATA_DIR=/data
ENV AUDION_PORT=8080
ENV RUST_LOG=info

# Define data volume
VOLUME /data

# Run application
CMD ["/app/audion-server"]
