# Port Forwarding & Pinggy-like Tunnel Setup for eagleoneonline.ca

This guide helps you expose your local ember-server (QUIC on UDP 4433) so external clients (e.g. Android app) can connect. **eagleoneonline.ca** is your server at `162.221.207.169` (hosting Punch Rendezvous).

---

## Two Approaches

| Approach | When to use | Home router config | VPS config |
|----------|-------------|--------------------|------------|
| **A. Direct port forwarding** | Simple, home IP is stable or you use DDNS | Forward UDP 4433 | DNS only |
| **B. Tunnel (Pinggy-like)** | Home IP changes often, or you want a central entry point | None (outbound only) | Run frps + open ports |

---

## Option A: Direct Port Forwarding

Expose your home PC directly. Android connects to `ember.eagleoneonline.ca:4433` → your home public IP → router forwards to PC.

### 1. Home router: forward UDP 4433

1. Log into your router (often `192.168.1.1` or `192.168.0.1`).
2. Find **Port Forwarding** / **Virtual Server** / **NAT**.
3. Add a rule:
   - **External port**: 4433 (UDP)
   - **Internal IP**: `192.168.1.238` (your PC)
   - **Internal port**: 4433
   - **Protocol**: UDP

### 2. DNS: point subdomain to your home IP

Create an A record:

```
ember.eagleoneonline.ca  →  <your-home-public-IP>
```

Find your public IP: https://whatismyip.com or `curl ifconfig.me`.

### 3. Dynamic DNS (if your home IP changes)

If your ISP assigns a dynamic IP, use a DDNS provider:

- **DuckDNS** (free): https://www.duckdns.org
- **No-IP**: https://www.noip.com
- **Cloudflare** (if you use it for eagleoneonline.ca): API-based updates

Then point `ember.eagleoneonline.ca` to the DDNS hostname (CNAME) or update the A record via the provider’s API.

### 4. Test

```bash
# On home PC
cargo run -p ember-server

# On Android app
Server: ember.eagleoneonline.ca:4433
```

---

## Option B: Tunnel (Pinggy-like) via eagleoneonline.ca

Run a tunnel server on your VPS. Your home PC runs a client that connects outbound (no router port forwarding). External clients connect to the VPS, which forwards traffic to your PC.

**[frp](https://github.com/fatedier/frp)** supports UDP and works well for QUIC.

### 1. Port forwarding on eagleoneonline.ca (VPS)

You need to open ports on the machine at `162.221.207.169`:

#### Cloud provider (AWS, Linode, DigitalOcean, etc.)

1. Open the **Security Group** or **Firewall** for the instance.
2. Add inbound rules:
   - **UDP 7000** – frp control channel
   - **UDP 4433** – ember QUIC (or another port if you prefer)

#### Linux firewall (ufw)

```bash
sudo ufw allow 7000/udp
sudo ufw allow 4433/udp
sudo ufw reload
```

#### Linux firewall (firewalld)

```bash
sudo firewall-cmd --permanent --add-port=7000/udp
sudo firewall-cmd --permanent --add-port=4433/udp
sudo firewall-cmd --reload
```

### 2. Install and run frps (server) on eagleoneonline.ca

```bash
# Download frp (replace with latest from https://github.com/fatedier/frp/releases)
wget https://github.com/fatedier/frp/releases/download/v0.52.3/frp_0.52.3_linux_amd64.tar.gz
tar xzf frp_0.52.3_linux_amd64.tar.gz
cd frp_0.52.3_linux_amd64
```

**frps.toml** (on the VPS):

```toml
bindPort = 7000
```

Run:

```bash
./frps -c frps.toml
```

(Use systemd/supervisor for production.)

### 3. Install and run frpc (client) on your home PC

**frpc.toml** (on your home PC):

```toml
serverAddr = "162.221.207.169"
serverPort = 7000

[[proxies]]
name = "ember-quic"
type = "udp"
localIP = "127.0.0.1"
localPort = 4433
remotePort = 4433
```

Run:

```bash
./frpc -c frpc.toml
```

### 4. DNS

Ensure `ember.eagleoneonline.ca` (or another subdomain) points to `162.221.207.169` (A record).

### 5. Test

```bash
# On home PC: run ember-server and frpc
cargo run -p ember-server   # in one terminal
./frpc -c frpc.toml         # in another

# On Android app
Server: ember.eagleoneonline.ca:4433
```

---

## Summary

| Goal | Action |
|------|--------|
| **Direct exposure** | Home router: forward UDP 4433. DNS: A record to home IP. Optional DDNS. |
| **Tunnel via VPS** | VPS: open UDP 7000, 4433. Run frps on VPS, frpc on home PC. DNS: A record to VPS IP. |

---

## Security notes

1. **ember-server**: Uses a self-signed cert; clients skip verification. For production, use a real cert or pin the certificate.
2. **frp**: Add `auth.token` in frps.toml and frpc.toml to restrict who can create tunnels.
3. **Firewall**: Only open the ports you need; avoid exposing extra services.
