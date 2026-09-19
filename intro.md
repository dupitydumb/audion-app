Hey Audion Community! 👋

If you've been running a homelab, self-hosting your services, or just tired of your music being held hostage by a subscription — this one's for you.

We're officially launching **Audion Self-Hosting** with `audion-server-docker`. Your music. Your server. Your rules. 🐳

---

## 🏠 Why Self-Host Your Music?

Most of us in the homelab world already know the answer. But here's the pitch for everyone else:

Streaming services come and go. Catalogs shrink. Prices go up. And somewhere between the algorithm and the licensing deals, *your* taste gets lost in a sea of recommendations you didn't ask for.

Self-hosting is different. You rip your CDs, buy your FLACs, rescue your MP3 collection from that dusty external drive — and you own it. No middleman. No monthly fee. No one deciding what's "available in your region."

Audion Server is built for exactly that. It's a lightweight audio streaming backend written in **Rust**, paired with a clean **Svelte 5** web interface. Toss it on a Raspberry Pi, an old PC, a VPS, or your NAS — and it just runs.

---

## ✨ What You Get

Once your server is up, here's what's waiting:

**🎵 Direct High-Quality Streaming**
Stream FLAC, ALAC, AAC, MP3, M4A and more from your own hardware to any device. No transcoding unless you want it.

**⚡ Real-Time Library Sync**
Uses Server-Sent Events (SSE) to push library updates — new tracks, deletions, changes — instantly to every connected client. No refresh needed.

**💾 Smart Metadata + Deduplication**
Upload and the server handles the rest. It reads your tags (album, artist, track number, genre, duration), extracts artwork, and deduplicates by content hash so your library stays clean even after messy imports.

**🖼️ Auto Cover Art**
Embedded artwork is extracted and served as proper web URLs — no extra setup.

**✍️ Synced Lyrics**
No lyrics in your file tags? The server quietly fetches them from [LRCLIB](https://lrclib.net/) — synchronized, timed lyrics that just appear.

**📻 Subsonic API Support**
Already using DSub, Symfonium, Feishin, or any other Subsonic-compatible client? Point it at your Audion server. It speaks the same protocol. Works on Android, iOS, and desktop out of the box.

**🔒 It's Just Yours**
No accounts on external clouds. No analytics. No one watching your listening habits. Your tracks, playlists, and history live on your hardware and nowhere else.

---

## 🛠️ Getting It Running

The whole stack ships as a single Docker image — nginx serving the web frontend, Rust backend running internally. One container, one port, done.

**Prerequisites:** Docker (and optionally Docker Compose)

### Step 1 — Clone and configure

```bash
git clone https://github.com/dupitydumb/audion-server-docker.git
cd audion-server-docker
cp .env.example .env
```

Open `.env` and set your credentials:

```env
AUDION_ADMIN_USER=admin
AUDION_ADMIN_PASSWORD=your-secure-password   # change this
AUDION_JWT_SECRET=a-long-random-string        # change this
```

> **Note:** Never commit your `.env` file. It's already in `.gitignore`.

### Step 2 — Start it

```bash
docker compose up --build -d
```

That's it. The image builds the frontend and backend together, starts the container, and serves everything on port **80**.

> Watch logs: `docker compose logs -f`  
> Stop: `docker compose down`

### Step 3 — Open the dashboard

Navigate to `http://localhost` (or your server's IP / custom domain) and log in with the credentials you set in `.env`.

From there you can upload music, scan your library, manage playlists, and configure users.

---

## 📱 Connecting the Audion App

Once your server is running, open the Audion app and:

1. Go to **Connect / Sync**
2. Select **Self-Hosted / Custom Server**
3. Enter:
   - **Server URL:** `http://<YOUR_SERVER_IP>` — or your custom domain (e.g. `https://music.yourdomain.com`)
   - **Username** and **Password** from your `.env`
4. Hit **Connect**

The app authenticates, opens the SSE stream for live updates, and loads your library. You're in.

---

## 🌐 Deploying with a Custom Domain (Coolify, Nginx Proxy Manager, etc.)

Running behind a reverse proxy? The container listens on port **80** and handles `/api/` and `/rest/` internally. Just point your proxy at the container — SSL termination happens at the proxy layer, nothing changes inside the image.

Subsonic clients connect to `https://music.yourdomain.com` — no port needed, no `/rest/` suffix.

---

## 💬 Show Off Your Setup

Got it running on a Pi? A homelab cluster? A $4/mo VPS? We want to see it. Drop your setup in our Discord — configs, screenshots, rack photos, all of it. 🎧

The homelab community built a lot of what makes self-hosting worth doing. This is our small contribution back.

Welcome to the stack. 🎶
