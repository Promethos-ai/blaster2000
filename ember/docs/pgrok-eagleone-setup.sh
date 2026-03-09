#!/bin/bash
# pgrok setup for eagleoneonline.ca - coexists with existing Apache
# Run on the server: curl -fsSL <url> | sudo bash
# Or: sudo bash pgrok-eagleone-setup.sh

set -euo pipefail

DOMAIN="eagleoneonline.ca"
ACME_EMAIL="${ACME_EMAIL:-danieljamesbertrand@me.com}"
SSH_PUB_KEY="${SSH_PUB_KEY:-}"

# Must be run as root
[[ "$(id -u)" -eq 0 ]] || { echo "Run with sudo"; exit 1; }

# Get SSH key if not set
if [[ -z "$SSH_PUB_KEY" ]]; then
  echo "Paste your SSH public key (from cat ~/.ssh/id_ed25519.pub):"
  read -r SSH_PUB_KEY
  [[ -n "$SSH_PUB_KEY" ]] || { echo "SSH key required"; exit 1; }
fi

echo "=== Step 1: Move Apache to port 8080 ==="
# Backup and modify Apache - Caddy will take over 80/443
PORTS_CONF="/etc/apache2/ports.conf"
if ! grep -q "Listen 8080" "$PORTS_CONF" 2>/dev/null; then
  cp "$PORTS_CONF" "${PORTS_CONF}.bak.$(date +%Y%m%d)"
  sed -i 's/^Listen 80$/Listen 8080/' "$PORTS_CONF"
  sed -i 's/^[[:space:]]*Listen 443$/	# Listen 443 - Caddy uses 443/' "$PORTS_CONF"
  # Update VirtualHosts to use 8080
  for f in /etc/apache2/sites-enabled/*.conf; do
    [ -f "$f" ] || continue
    sed -i 's/<VirtualHost \*:80>/<VirtualHost *:8080>/g' "$f"
    sed -i 's/<VirtualHost \*:443>/<VirtualHost *:8080>/g' "$f"
  done
  systemctl reload apache2
  echo "Apache now on 8080"
else
  echo "Apache already on 8080"
fi

echo "=== Step 2: Install pgrok server components ==="
CLONE_DIR="/opt/pgrok/repo"
mkdir -p "$(dirname "$CLONE_DIR")"
if [[ ! -d "$CLONE_DIR/.git" ]]; then
  git clone --depth 1 https://github.com/R44VC0RP/pgrok.git "$CLONE_DIR"
fi

SERVER_DIR="/opt/pgrok"
mkdir -p "$SERVER_DIR"

# Copy Docker files
cp "${CLONE_DIR}/server/Dockerfile" "$SERVER_DIR/"
cp "${CLONE_DIR}/server/docker-compose.yml" "$SERVER_DIR/"

# Custom Caddyfile: Apache proxy + pgrok on-demand TLS
cat > "${SERVER_DIR}/Caddyfile" << 'CADDYEOF'
{
  on_demand_tls {
    ask http://localhost:9123/check
  }
  email ACME_EMAIL_PLACEHOLDER
}

# HTTP -> HTTPS redirect
http:// {
  redir https://{host}{uri} permanent
}

# Main sites: proxy to Apache on 8080
eagleoneonline.ca, www.eagleoneonline.ca, supplementsnow.ca {
  reverse_proxy 127.0.0.1:8080
}

# pgrok tunnels: on-demand TLS for *.eagleoneonline.ca (subdomains only)
https:// {
  tls {
    on_demand
    issuer acme
    issuer acme {
      dir https://acme.zerossl.com/v2/DV90
    }
  }
}
CADDYEOF
sed -i "s/ACME_EMAIL_PLACEHOLDER/${ACME_EMAIL}/" "${SERVER_DIR}/Caddyfile"

# Install pgrok-ask
sed "s/yourdomain\.com/${DOMAIN}/g" "${CLONE_DIR}/server/pgrok-ask" > /usr/local/bin/pgrok-ask
chmod +x /usr/local/bin/pgrok-ask
sed "s/yourdomain\.com/${DOMAIN}/g" "${CLONE_DIR}/server/pgrok-ask.service" > /etc/systemd/system/pgrok-ask.service
systemctl daemon-reload
systemctl enable pgrok-ask
systemctl restart pgrok-ask

# Install pgrok-tunnel
sed "s/yourdomain\.com/${DOMAIN}/g" "${CLONE_DIR}/server/pgrok-tunnel" > /usr/local/bin/pgrok-tunnel
chmod +x /usr/local/bin/pgrok-tunnel

# Create pgrok user and add SSH key
PGROK_USER="pgrok"
id "$PGROK_USER" &>/dev/null || useradd -m -s /bin/bash "$PGROK_USER"
SSH_DIR="/home/${PGROK_USER}/.ssh"
mkdir -p "$SSH_DIR"
echo "$SSH_PUB_KEY" >> "${SSH_DIR}/authorized_keys"
chown -R "${PGROK_USER}:${PGROK_USER}" "$SSH_DIR"
chmod 700 "$SSH_DIR"
chmod 600 "${SSH_DIR}/authorized_keys"

# Configure sshd for pgrok
if ! grep -q "# pgrok tunnel" /etc/ssh/sshd_config; then
  cat >> /etc/ssh/sshd_config << 'SSHDEOF'

# pgrok tunnel configuration
Match User pgrok
  AllowTcpForwarding remote
  GatewayPorts no
  X11Forwarding no
  PermitTTY yes
SSHDEOF
  systemctl restart sshd 2>/dev/null || systemctl restart ssh
fi

echo "=== Step 3: Start Caddy ==="
cd "$SERVER_DIR"
docker compose up -d --build 2>/dev/null || docker-compose up -d --build

echo ""
echo "=== pgrok server ready on eagleoneonline.ca ==="
echo ""
echo "Client setup (run on your Mac/Linux):"
echo "  curl -fsSL https://raw.githubusercontent.com/R44VC0RP/pgrok/main/install.sh | bash -s client"
echo ""
echo "When prompted, use:"
echo "  VPS: eagleoneonline.ca"
echo "  Domain: eagleoneonline.ca"
echo "  SSH user: pgrok"
echo ""
echo "Then expose a local service:"
echo "  pgrok myapp 4000   # => https://myapp.eagleoneonline.ca -> localhost:4000"
echo ""
