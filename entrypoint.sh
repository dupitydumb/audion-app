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
echo "Fixing ownership for /data and /app..."
chown -R audion:audion /data /app

# Print getting started info
AUDION_PORT=${AUDION_PORT:-8080}
AUDION_ADMIN_USER=${AUDION_ADMIN_USER:-admin}

echo ""
echo "========================================================================"
echo "  Audion Server is ready!"
echo "========================================================================"
echo "  🌐 Web UI / API:       http://localhost:${AUDION_PORT}"
echo "  🔑 Admin Username:     ${AUDION_ADMIN_USER}"
echo "  📁 Data Directory:     /data (SQLite DB & settings)"
echo "  🎵 Music Directory:    /data/users/<username>/"
echo ""
echo "  To get started:"
echo "  1. Open http://localhost:${AUDION_PORT} in your web browser."
echo "  2. Log in using the admin credentials from your .env file."
echo "  3. Upload/drop your music files under the users' directory."
echo "========================================================================"
echo ""

# Drop privileges and run the main command
echo "Launching audion-server as non-root user..."
exec gosu audion /app/audion-server "$@"
