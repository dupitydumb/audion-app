# Build stage
FROM rust:1.89-slim-bookworm AS builder

# Install system dependencies (build-essential, pkg-config, etc. if needed)
RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    pkg-config \
    libssl-dev \
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
    gosu \
    && rm -rf /var/lib/apt/lists/*

# Copy compiled binary from builder
COPY --from=builder /app/target/release/audion-server /app/audion-server

# Expose server port
EXPOSE 8080

# Set environment defaults
ENV AUDION_DATA_DIR=/data
ENV AUDION_PORT=8080
ENV RUST_LOG=info
ENV AUDION_ADMIN_USER=admin
ENV AUDION_ADMIN_PASSWORD=changeme
ENV AUDION_JWT_SECRET=your-secret-key-here-change-this-in-production
ENV AUDION_JWT_EXPIRATION_DAYS=7
ENV AUDION_CORS_ORIGIN=*
ENV AUDION_MAX_BODY_SIZE=262144000
ENV AUDION_PUBLIC_DIR=/app/frontend/dist

# Define data volume
VOLUME /data

# Create non-root user and group
RUN groupadd -g 10001 audion && \
    useradd -u 10001 -g audion -m -s /usr/sbin/nologin audion

# Ensure directories exist and have proper ownership
RUN mkdir -p /data && chown -R audion:audion /app /data

# Copy entrypoint script and make it executable
COPY entrypoint.sh /app/entrypoint.sh
RUN chmod +x /app/entrypoint.sh

# The entrypoint will start as root, fix permissions, and then run as 'audion'
ENTRYPOINT ["/app/entrypoint.sh"]


