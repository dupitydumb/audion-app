# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- MIT License file and README license badge.
- Detailed JSON health check endpoint (`/api/health`) reporting database status, app version, and timestamp.
- Docker Compose health check configuration for `audion-server`.
- `.env.example` file to manage configuration variables and secret keys securely.
- Multi-architecture Docker image builds (`linux/amd64` and `linux/arm64`) using QEMU emulation in GitHub Actions.
- `CONTRIBUTING.md` guide for FOSS development setup, coding standards, and contributions.
- `SECURITY.md` detailing supported versions and responsible security vulnerability disclosure policy.
- GitHub Issue templates (`bug_report.md`, `feature_request.md`) and Pull Request template.

### Changed
- Refactored `docker-compose.yml` to use `env_file: .env` instead of hardcoding environment variables inline.
- Upgraded S3 storage credential storage by encrypting `s3_secret_key` at rest in SQLite database using AES-GCM (reusing JWT secret). Masked secret key in storage settings API responses.

### Fixed
- Implemented rate limiting (10 requests per minute per IP) on `/api/auth/login` to prevent brute-force attacks.
- Fixed dead tunnel process detection by replacing the no-op 5-second sleep placeholder with a real `tokio::select!` child process monitor across all tunnel providers (LocalhostRun, Ngrok, Cloudflare).
- Cleaned up rendering fences around YAML and bash snippets in `intro.md`.
