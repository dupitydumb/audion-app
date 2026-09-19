#!/bin/sh
set -e

# Get PUID and PGID from env or default to 10001 (audion)
PUID=${PUID:-10001}
PGID=${PGID:-10001}

echo "Starting Audion Server entrypoint..."
echo "UID: $PUID, GID: $PGID"

# Modify audion group GID if it differs from the desired PGID
if [ "$(id -g audion)" -ne "$PGID" ]; then
    echo "Updating group audion GID to $PGID..."
    groupmod -o -g "$PGID" audion
fi

# Modify audion user UID if it differs from the desired PUID
if [ "$(id -u audion)" -ne "$PUID" ]; then
    echo "Updating user audion UID to $PUID..."
    usermod -o -u "$PUID" audion
fi

# Ensure correct permissions on data and application directories
if [ "$PUID" != "10001" ] || [ "$PGID" != "10001" ]; then
    echo "Fixing ownership for /data and /app..."
    chown -R audion:audion /data /app
fi

# Ensure data subdirectories exist with correct ownership
mkdir -p /data/db /data/tracks /data/artwork
chown -R audion:audion /data

# Start backend as audion user in background
echo "Launching audion-server as non-root user (background)..."
gosu audion /app/audion-server &

# Wait for backend to bind before nginx starts proxying (max 30s)
echo "Waiting for backend to be ready..."
WAIT=0
until curl -sf http://127.0.0.1:8080/api/health > /dev/null 2>&1; do
    if [ "$WAIT" -ge 30 ]; then
        echo "ERROR: backend did not start within 30s" >&2
        exit 1
    fi
    sleep 1
    WAIT=$((WAIT + 1))
done
echo "Backend ready."

# Start nginx as PID 1 (foreground)
echo "Starting nginx..."
exec nginx -g "daemon off;"
