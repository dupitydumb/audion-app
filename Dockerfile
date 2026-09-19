# ── Stage 1: Build frontend ────────────────────────────────────────────────
FROM node:20-alpine AS node-builder

WORKDIR /app

COPY frontend/package.json frontend/package-lock.json ./
RUN npm install

COPY frontend/ ./
RUN npm run build
# Output: /app/dist

# ── Stage 2: Build backend ─────────────────────────────────────────────────
FROM rust:1.92-slim-bookworm AS rust-builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    build-essential \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Pre-compile deps (cache layer)
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release
RUN rm -rf src

# Build real binary
COPY src ./src
RUN touch src/main.rs
RUN cargo build --release
# Output: /app/target/release/audion-server

# ── Stage 3: Runtime ───────────────────────────────────────────────────────
FROM debian:bookworm-slim

WORKDIR /app

# Install runtime dependencies
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    sqlite3 \
    ffmpeg \
    nginx \
    gosu \
    openssh-client \
    curl \
    gnupg \
    # cloudflared pinned to 2024.12.2 for reproducible builds; update deliberately
    && ARCH=$(dpkg --print-architecture) \
    && curl -L -o /tmp/cloudflared.deb "https://github.com/cloudflare/cloudflared/releases/download/2024.12.2/cloudflared-linux-${ARCH}.deb" \
    && dpkg -i /tmp/cloudflared.deb \
    && rm /tmp/cloudflared.deb \
    && curl -s https://ngrok-agent.s3.amazonaws.com/ngrok.asc | tee /etc/apt/trusted.gpg.d/ngrok.asc >/dev/null \
    && echo "deb https://ngrok-agent.s3.amazonaws.com/ buster main" | tee /etc/apt/sources.list.d/ngrok.list \
    && apt-get update \
    # ponytail: ngrok version unpinned via apt; pin by switching to direct binary download when supply-chain matters
    && apt-get install -y ngrok \
    && rm -rf /var/lib/apt/lists/*

# Copy backend binary
COPY --from=rust-builder /app/target/release/audion-server /app/audion-server

# Copy frontend static files
COPY --from=node-builder /app/dist /usr/share/nginx/html

# Copy nginx config
COPY nginx.conf /etc/nginx/conf.d/default.conf
# Remove default nginx site
RUN rm -f /etc/nginx/sites-enabled/default

# Expose only port 80 (nginx); backend runs on 8080 internally
EXPOSE 80

# Set environment defaults
ENV AUDION_DATA_DIR=/data
ENV AUDION_PORT=8080
ENV RUST_LOG=info
ENV AUDION_JWT_EXPIRATION_DAYS=7
ENV AUDION_CORS_ORIGIN=*
ENV AUDION_MAX_BODY_SIZE=262144000

# Define data volume
VOLUME /data

# Create non-root user for backend
RUN groupadd -g 10001 audion && \
    useradd -u 10001 -g audion -m -s /usr/sbin/nologin audion

# Ensure directories exist with proper ownership
RUN mkdir -p /data && chown -R audion:audion /app /data

# Copy entrypoint script
COPY entrypoint.sh /app/entrypoint.sh
RUN chmod +x /app/entrypoint.sh

ENTRYPOINT ["/app/entrypoint.sh"]
