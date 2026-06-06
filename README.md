# 🎵 Audion Server

A lightweight, self-hosted audio streaming server written in Rust with a modern, responsive Svelte frontend. Audion Server enables you to upload, manage, index, and stream your personal music library seamlessly.

![License](https://img.shields.io/github/license/dupitydumb/audion-server-docker?style=for-the-badge)
![Rust](https://img.shields.io/badge/rust-%23E34F26.svg?style=for-the-badge&logo=rust&logoColor=white)
![Svelte](https://img.shields.io/badge/svelte-%23FF3E00.svg?style=for-the-badge&logo=svelte&logoColor=white)
![Docker](https://img.shields.io/badge/docker-%230db7ed.svg?style=for-the-badge&logo=docker&logoColor=white)

---

## 🚀 Key Features

*   **Smart Metadata Scanning**: Automatically extracts album, artist, track title, track number, disc number, genre, and duration from audio tags using `lofty` and `metaflac`.
*   **Supported Formats**: Full support for `FLAC`, `ALAC`, `AAC`, `MP3`, `M4A`, and more.
*   **Content-Based Deduplication**: Automatically prevents duplicate uploads by calculating a unique content hash from track metadata (title, artist, album, and duration).
*   **Auto Artwork Extraction**: Extract and store embedded track covers and album art, making them available as web-friendly URLs.
*   **Dynamic Playlists**: Create, edit, and organize custom playlists with full drag-and-drop track reordering support.
*   **Synced Lyrics Retrieval**: Integrates with [LRCLIB](https://lrclib.net/) to automatically retrieve synchronized and plain text lyrics when they are not present in local tags.
*   **Real-Time Client Sync**: Uses Server-Sent Events (SSE) to broadcast library updates (e.g. `track.added`, `track.deleted`) to keep client interfaces synchronized.
*   **Secure Authentication**: JWT-based authentication system with auto-bootstrapping for the administrator user.
*   **Subsonic Client Support**: Exposes a standard Subsonic-compliant API (under `/rest/`) for compatibility with external music streaming applications on Android, iOS, and Desktop.

---

## 🛠️ Architecture

Audion Server is structured as a monorepo containing:

*   **Backend (`/src`)**: Rust web server built using:
    *   `axum` for HTTP API routing and Server-Sent Events (SSE).
    *   `sqlx` (SQLite) for database storage and indexing.
    *   `lofty` and `metaflac` for advanced audio metadata parsing.
    *   `argon2` for secure password hashing.
    *   `reqwest` for external lyric fetching.
*   **Frontend (`/frontend`)**: Single Page Application built using:
    *   `Svelte` (v5) for reactive UI architecture.
    *   `Vite` for development tooling and static asset compilation.
    *   `lucide-svelte` for clean iconography.
    *   Vanilla CSS variables for beautiful dark-mode glassmorphism and responsiveness.

---

## ⚙️ Configuration

Audion Server is configured using environment variables. When starting, the backend will attempt to load the following variables:

| Environment Variable | Description | Default Value |
| --- | --- | --- |
| `AUDION_PORT` | The port the backend server listens on | `8080` |
| `AUDION_ADMIN_USER` | Initial username for the bootstrapped administrator | `admin` |
| `AUDION_ADMIN_PASSWORD` | Initial password for the bootstrapped administrator | `changeme` |
| `AUDION_JWT_SECRET` | Secret key used for signing JWT tokens | `your-secret-key-here-super-secure` |
| `AUDION_DATA_DIR` | Directory where SQLite database, tracks, and artwork are stored | `./data` |
| `AUDION_PUBLIC_DIR` | Directory containing built frontend static files (for single-binary mode) | `./frontend/dist` |
| `AUDION_JWT_EXPIRATION_DAYS` | Number of days before a issued JWT token expires | `7` |
| `AUDION_CORS_ORIGIN` | CORS allowed origins (e.g. `*` or a specific URL) | `*` |
| `AUDION_MAX_BODY_SIZE` | Maximum allowed request body size in bytes (e.g. for uploads) | `262144000` (250MB) |
| `RUST_LOG` | Backend logging level (`trace`, `debug`, `info`, `warn`, `error`) | `info` |

---

## 📦 Getting Started & Setup

### Option 1: Running with Docker (Recommended)

Audion Server comes with a preconfigured `docker-compose.yml` for multi-container orchestration.

1.  **Verify or Edit `docker-compose.yml`**
    Ensure the ports are mapped correctly on your host machine. For example, to expose the frontend on port `80` and the backend on `8080`, check or edit `docker-compose.yml` to specify:
    ```yaml
    version: '3.8'

    services:
      audion-server:
        build:
          context: .
          dockerfile: Dockerfile
        ports:
          - "8080:8080" # Map backend port to host
        volumes:
          - ./data:/data
        environment:
          - AUDION_ADMIN_USER=admin
          - AUDION_ADMIN_PASSWORD=securepasswordhere # Change this!
          - AUDION_JWT_SECRET=use-a-strong-jwt-secret-here # Change this!
          - AUDION_DATA_DIR=/data
          - AUDION_PORT=8080
          - RUST_LOG=info
        restart: unless-stopped

      audion-frontend:
        build:
          context: ./frontend
          dockerfile: Dockerfile
        ports:
          - "80:80" # Map frontend port to host
        depends_on:
          - audion-server
        restart: unless-stopped
    ```

2.  **Start the Containers**
    Run the following command in the root directory:
    ```bash
    docker compose up --build -d
    ```
    This compiles the Rust backend inside a slim Debian image, builds the Svelte frontend, and launches the services with Nginx routing requests from `/api/` to the backend.

3.  **Access the Application**
    Open your browser and navigate to `http://localhost`. Log in using your configured administrator credentials.

> [!TIP]
> **Permission Issues with Database Writes (SQLite Error Code 8)**
> If you encounter a `500 Internal Server Error` with `attempt to write a readonly database` when performing writes (e.g., toggling liked tracks, creating playlists), it means the SQLite database files or subdirectories under `/data` are owned by `root` instead of the non-root `audion` user (UID 10001) that the server runs as.
>
> You can fix this by running the following command to correct ownership in the running container:
> ```bash
> docker exec -u root audion-server-docker-audion-server-1 chown -R audion:audion /data
> ```

---

### Option 2: Local Development Setup

If you want to run the server directly on your host machine for development:

#### Prerequisites

*   **Rust Toolchain**: [Install Rust](https://www.rust-lang.org/tools/install) (latest stable release)
*   **Node.js & npm**: [Install Node.js](https://nodejs.org/) (v18+)

#### 1. Setup the Frontend

Navigate into the `frontend` folder, install dependencies, and build or run the development server:

```bash
cd frontend
npm install

# Option A: Run Vite development server (runs with hot reload at http://localhost:5173)
npm run dev

# Option B: Build static assets to let the Rust backend serve them
npm run build
```

#### 2. Run the Backend

If you are running the frontend via its development server (Option A), navigate to the root directory and start the backend:

```bash
# Set your environment variables (optional, defaults will be used)
# On Windows PowerShell:
$env:AUDION_PORT="8080"

# On Linux/macOS:
export AUDION_PORT=8080

# Launch the Rust server
cargo run
```

> [!NOTE]
> If you built the frontend using `npm run build`, the Rust server will automatically serve the static files from `./frontend/dist` on `http://localhost:8080` (no Nginx or separate node server required).

---

## 📂 Project Directory Structure

```
audion-server/
├── .github/workflows/    # CI/CD Workflows (Docker build/publish to GHCR)
├── src/                  # Rust Backend Source
│   ├── api/              # Axum handlers (auth, tracks, playlists, stream...)
│   ├── auth/             # JWT auth validation & password hashing
│   ├── db/               # SQLite database pool initializer & setup
│   ├── events/           # Event bus for Server-Sent Events (SSE)
│   ├── scanner/          # Audio parsing engine (`lofty` / `metaflac`)
│   ├── config.rs         # Environment variable loader
│   ├── state.rs          # Axum Application State
│   └── main.rs           # Server entry point
├── frontend/             # Svelte Frontend Source
│   ├── src/
│   │   ├── components/   # UI Modules (Library, Albums, Artists, Playlists, Player...)
│   │   ├── App.svelte    # Frontend routing & layout wrapper
│   │   └── app.css       # Custom Glassmorphism Styles & animations
│   ├── Dockerfile        # Frontend multi-stage build (Node + Nginx)
│   └── nginx.conf        # Nginx route router and API reverse-proxy
├── Dockerfile            # Backend multi-stage build (Slim Debian)
├── docker-compose.yml    # Combined stack setup
└── Cargo.toml            # Rust dependency manifest
```

## 📻 Subsonic Client Integration

Audion Server includes a built-in Subsonic-compatible API, allowing you to connect and stream your music library to any Subsonic-compatible client app. 

### Recommended Clients
*   **Android:** Symfonium, DSub, Substreamer, UltraSonic
*   **iOS:** play:Sub, Substreamer, Amuse, AVSub
*   **Desktop/Web:** Feishin, Sonixd, Sublime Music

### Connection Details
To connect a Subsonic client to your Audion Server, configure the following connection parameters:
1.  **Server URL**: `http://<your-server-ip>:<port>` (e.g., `http://localhost:8080`). Do not append `/rest/` as clients append this automatically.
2.  **Username**: Your Audion username.
3.  **Password**: Your Audion password.

The server supports standard plain-text password authentication as well as secure salt/token-based MD5 authentication out of the box.

---

## 🔒 Security & Best Practices

*   Default credentials are bootstrapped on the database's first run. **Make sure to change the default admin password (`changeme`)** by updating the environment variables before deploying.
*   Ensure `AUDION_JWT_SECRET` is set to a unique, random string in production environments to secure auth tokens.
