# Contributing to Audion Server

Thank you for your interest in contributing to Audion Server! We welcome bug reports, feature requests, documentation improvements, and pull requests.

## 🛠️ Development Prerequisites

To develop on the codebase, you need:
- **Rust Stable** (v1.82+)
- **Node.js** (v18+)
- **Docker** and **Docker Compose** (optional, for containerised testing)
- **FFmpeg** installed and added to your system path (optional, for FLAC transcoding support)

## 💻 Local Setup

1. **Clone the repository:**
   ```bash
   git clone https://github.com/your-username/audion-server-docker.git
   cd audion-server-docker
   ```

2. **Configure environment:**
   Copy the example environment file:
   ```bash
   cp .env.example .env
   ```
   Generate a JWT secret:
   ```bash
   openssl rand -hex 32
   ```
   Open `.env` and set `AUDION_JWT_SECRET` to the generated hex string, and `AUDION_ADMIN_PASSWORD` to a password of your choice.

3. **Start the Frontend Development Server:**
   ```bash
   cd frontend
   npm install
   npm run dev
   ```
   The frontend will run at `http://localhost:5173` with hot-module reloading.

4. **Start the Backend:**
   In another terminal, return to the root folder and run:
   ```bash
   cargo run
   ```
   The backend will run on `http://localhost:8080`.

## 🎨 Code Styling & Quality

We maintain high code quality standards. Please check the following before submitting code changes:
- **Rust Backend**:
  Format your code:
  ```bash
  cargo fmt
  ```
  Run linting checks:
  ```bash
  cargo clippy --all-targets --all-features -- -D warnings
  ```
- **Svelte Frontend**:
  Format and lint the frontend files:
  ```bash
  cd frontend
  npm run lint
  ```

## 🧪 Testing

Before creating a pull request, run the local integration tests to verify your changes do not break existing features:
```bash
# Run backend compile and cargo check
cargo check

# Run the API/integration test suite
node tests/api_test.js
```

## 📬 Pull Request Process

1. Fork the repository and create your branch from `master` (e.g., `feature/cool-new-thing` or `fix/tunnel-crash`).
2. Implement your changes, adding tests if applicable.
3. Ensure all styling, linting, and tests pass.
4. Commit your changes with clear, descriptive commit messages.
5. Push to your fork and submit a Pull Request.
6. A maintainer will review your pull request shortly!
