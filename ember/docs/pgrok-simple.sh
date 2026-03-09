#!/bin/bash
# Minimal pgrok client - no TUI, just SSH tunnel
# Usage: ./pgrok-simple.sh myapp 4000
# Requires: ~/.pgrok/config (or set PGROK_HOST, PGROK_DOMAIN, PGROK_USER, PGROK_SSH_KEY)
# DNS: Add wildcard A record *.eagleoneonline.ca -> 162.221.207.169

set -e

[[ $# -ge 2 ]] || { echo "Usage: $0 <subdomain> <local_port>"; exit 1; }
SUBDOMAIN="${1,,}"
LOCAL_PORT="$2"

# Load config
CONFIG="${HOME}/.pgrok/config"
[[ -f "$CONFIG" ]] && source "$CONFIG"
: "${PGROK_HOST:=eagleoneonline.ca}"
: "${PGROK_DOMAIN:=eagleoneonline.ca}"
: "${PGROK_USER:=pgrok}"

# Compute remote port (must match pgrok server)
REMOTE_PORT=$(( 10000 + ($(printf '%s' "$SUBDOMAIN" | cksum | awk '{print $1}') % 50000) ))

echo "pgrok: https://${SUBDOMAIN}.${PGROK_DOMAIN} -> localhost:${LOCAL_PORT}"
echo "Starting tunnel (Ctrl+C to stop)..."

SSH_OPTS=(-T -o ServerAliveInterval=30 -o ServerAliveCountMax=3 -o ConnectTimeout=10 -o LogLevel=ERROR)
[[ -n "${PGROK_SSH_KEY:-}" ]] && SSH_OPTS+=(-i "$PGROK_SSH_KEY")

exec ssh "${SSH_OPTS[@]}" \
  -R "${REMOTE_PORT}:localhost:${LOCAL_PORT}" \
  "${PGROK_USER}@${PGROK_HOST}" \
  "PYTHONUNBUFFERED=1 /usr/local/bin/pgrok-tunnel ${SUBDOMAIN} ${REMOTE_PORT}"
