Hey Audion Community! 👋

We have some incredibly exciting news for the self-hosters, homelab enthusiasts, and privacy advocates out there! We are officially introducing Audion Self-Hosting using audion-server-docker! 🐳✨

If you’ve ever wanted to access your personal music library across all your devices, keep your data strictly private, and experience real-time library synchronization, this is the ultimate upgrade you've been waiting for.

Here is everything you need to know about the self-hosted Audion Server, its benefits, and how you can get started today!

🎵 Welcome to Audion Self-Hosting: Complete Control of Your Music
🏠 What is Self-Hosting?
By default, Audion operates as a fully local, privacy-focused offline player on your device. With our new Self-Hosted / Custom Server support, you can now run your own private Audion Server.

The Audion Server is a lightweight audio streaming backend written in Rust, paired with a modern, responsive web frontend built with Svelte 5. By running the server on your home computer, a Raspberry Pi, or a VPS, you create your own private cloud. Your Audion app then connects directly to your own server, keeping you in charge of your data and media!

✨ What Can You Do With It?
Once you pair your Audion app with your Audion Server, you unlock a suite of powerful features:

⚡ Real-Time Client Sync (SSE): The server uses Server-Sent Events (SSE) to instantly broadcast library updates (such as adding or deleting tracks) directly to your connected Audion client, keeping everything in perfect sync.
🎵 Direct Audio Streaming: Stream your high-quality audio files (FLAC, ALAC, AAC, MP3, M4A, and more) directly from your server. No need to keep music files duplicated on every device!
💾 Smart Metadata Scanning & Deduplication: When you upload music, the server automatically scans track tags (album, artist, track number, genre, duration) and performs content-based deduplication using metadata hashes to ensure a clean library.
🖼️ Auto Artwork Extraction: Embedded cover art is automatically extracted and served as web-friendly URLs.
✍️ Synced Lyrics on the Fly: Audion Server integrates with LRCLIB to automatically fetch synchronized and plain-text lyrics when they aren't embedded in your files.
📁 Offline Listening: If you're on the go, you can download/resolve tracks from your server directly into your local Audion app storage to listen offline.
🌟 What are the Benefits?
🔒 Complete Data Privacy: No third-party servers, no analytics trackers, no account registration on external clouds. Your tracks, playlists, and listening history stay on your hardware.
⚡ Lightweight & Fast: Built on a Rust web backend (axum) and SQLite (sqlx), the server is highly optimized. It runs smoothly with minimal CPU and memory footprints.
📱 Cross-Device Synchronized Experience: Connect multiple Audion client apps (Windows, macOS, Linux, Android) to the same server and enjoy a consistent, unified library.
🌐 Web Access Anywhere: The Docker setup bundles a standalone Svelte web app. Even without the desktop app, you can log in to your server via any web browser on port 80 to manage your library.
🛠️ Getting Started: How to Set Up & Connect
Deploying your server and connecting your app is quick and painless. Here is the step-by-step guide:

Step 1: Deploying the Audion Server with Docker
The easiest way to run the server is using Docker Compose.

Locate the docker-compose.yml file in the audion-server-docker directory. It defines two services:
audion-server (Rust backend listening on port 8080)
audion-frontend (Nginx + Svelte 5 frontend listening on port 80)
Edit the environment variables in your docker-compose.yml file to secure your setup:
yaml
environment:
  - AUDION_ADMIN_USER=admin
  - AUDION_ADMIN_PASSWORD=your-secure-password   # Change this!
  - AUDION_JWT_SECRET=your-custom-secure-secret   # Change this!
  - AUDION_PORT=8080
  - AUDION_DATA_DIR=/data
Run the following command in your terminal to build and start the containers in the background:
bash
docker compose up --build -d
Access the web dashboard by opening your browser and navigating to http://localhost (or your machine's IP address) and log in with your configured admin credentials. Here you can upload files and manage playlists.
Step 2: Connecting the Audion App
Once your server is running, connecting your desktop app takes seconds:

Open your Audion app.
Navigate to the Connect / Sync panel.
Under server settings, select Self-Hosted / Custom Server.
Enter your credentials:
Server URL: http://<YOUR_SERVER_IP>:8080 (use the backend port 8080 for API connection)
Username: (The AUDION_ADMIN_USER you configured)
Password: (The AUDION_ADMIN_PASSWORD you configured)
Click Connect Server.
Your client will switch provider modes to Server Mode, authenticate securely using a signed JWT token, initiate the Server-Sent Events listener for real-time updates, and load your custom library!

We are incredibly excited about this next step for Audion. If you run into any setup questions or want to show off your homelab configs, join us on our Discord! 🎧🎉