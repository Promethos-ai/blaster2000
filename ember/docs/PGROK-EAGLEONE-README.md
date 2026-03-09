# pgrok on eagleoneonline.ca (Pinggy-like Service)

A self-hosted tunnel service is running on **eagleoneonline.ca**, similar to Pinggy. Expose local HTTP services to the internet with automatic HTTPS.

## Status

- **Server**: Running on eagleoneonline.ca
- **Caddy**: Proxies eagleoneonline.ca, www, supplementsnow.ca → Apache (port 8080)
- **pgrok tunnels**: `*.eagleoneonline.ca` subdomains with on-demand TLS
- **SSH user**: `pgrok` (key-based auth only)

## DNS (Required for tunnels)

Add a **wildcard A record** so tunnel subdomains resolve:

| Type | Name | Value |
|------|------|-------|
| A | `*` | `162.221.207.169` |

This allows `myapp.eagleoneonline.ca`, `api.eagleoneonline.ca`, etc. to reach your server.

**Note:** If using Cloudflare, turn the proxy (orange cloud) **OFF** for the wildcard record so ACME challenges work.

## Client Setup

### Option A: Full client (Mac/Linux, requires Bun)

```bash
curl -fsSL https://raw.githubusercontent.com/R44VC0RP/pgrok/main/install.sh | bash -s client
```

When prompted: VPS `eagleoneonline.ca`, domain `eagleoneonline.ca`, user `pgrok`, SSH key `~/.ssh/id_ed25519`.

### Option B: Simple script (Windows/WSL, no Bun)

```bash
# In WSL (config at ~/.pgrok/config from a prior install attempt)
sed 's/\r$//' docs/pgrok-simple.sh > ~/pgrok-simple.sh
bash ~/pgrok-simple.sh myapp 4000
```

## Usage

```bash
# Expose local port 4000 as https://myapp.eagleoneonline.ca
pgrok myapp 4000

# Expose port 3000 as https://api.eagleoneonline.ca
pgrok api 3000
```

Press Ctrl+C to stop. The tunnel URL is removed automatically.

## Limitations

- **HTTP/HTTPS only** — TCP and UDP tunnels (e.g. ember QUIC) need [frp](docs/PORT-FORWARDING-AND-TUNNEL-SETUP.md) instead
- **Single user** — Your SSH key is the only authorized tunnel client
- **Mac/Linux client** — No Windows client yet (use WSL or a Linux VM)

## Server Management

```bash
# View Caddy logs
ssh dbertrand@eagleoneonline.ca "cd /opt/pgrok && sudo docker compose logs -f caddy"

# Restart Caddy
ssh dbertrand@eagleoneonline.ca "cd /opt/pgrok && sudo docker compose restart"

# Check pgrok-ask (cert validation)
ssh dbertrand@eagleoneonline.ca "sudo systemctl status pgrok-ask"
```

## Re-running Setup

The setup script is at `docs/pgrok-eagleone-setup.sh`. To re-run (e.g. to add another SSH key):

```bash
# Copy script and run with your SSH public key
scp docs/pgrok-eagleone-setup.sh dbertrand@eagleoneonline.ca:/tmp/
ssh dbertrand@eagleoneonline.ca "sudo SSH_PUB_KEY='$(cat ~/.ssh/id_ed25519.pub)' bash /tmp/pgrok-setup.sh"
```
