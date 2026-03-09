#!/bin/bash
# Complete pgrok setup: DNS wildcard + client install
# Usage:
#   CLOUDFLARE_API_TOKEN=xxx CLOUDFLARE_ZONE_ID=xxx ./pgrok-complete-setup.sh
#   Or without env vars: ./pgrok-complete-setup.sh  (DNS step will be skipped, instructions shown)

set -e

DOMAIN="eagleoneonline.ca"
IP="162.221.207.169"

echo "=== 1. DNS: Add wildcard A record ==="
if [[ -n "${CLOUDFLARE_API_TOKEN:-}" && -n "${CLOUDFLARE_ZONE_ID:-}" ]]; then
  echo "Adding *.${DOMAIN} -> ${IP} via Cloudflare API..."
  curl -s -X POST "https://api.cloudflare.com/client/v4/zones/${CLOUDFLARE_ZONE_ID}/dns_records" \
    -H "Authorization: Bearer ${CLOUDFLARE_API_TOKEN}" \
    -H "Content-Type: application/json" \
    --data "{\"type\":\"A\",\"name\":\"*\",\"content\":\"${IP}\",\"ttl\":1,\"proxied\":false}" | head -c 500
  echo ""
  echo "DNS record added. (Set proxied:false for ACME challenges.)"
else
  echo "Add this DNS record manually (Cloudflare, Namecheap, etc.):"
  echo "  Type: A"
  echo "  Name: *"
  echo "  Value: ${IP}"
  echo ""
  echo "For Cloudflare API: CLOUDFLARE_API_TOKEN=xxx CLOUDFLARE_ZONE_ID=xxx $0"
fi

echo ""
echo "=== 2. pgrok client install ==="
if [[ -f ~/.pgrok/config ]]; then
  echo "Config exists. Rebuilding..."
  bash ~/.pgrok/repo/setup.sh client --rebuild
else
  echo "Run: curl -fsSL https://raw.githubusercontent.com/R44VC0RP/pgrok/main/install.sh | bash -s client"
  echo "Use: eagleoneonline.ca, eagleoneonline.ca, pgrok, ~/.ssh/id_ed25519"
fi

echo ""
echo "=== 3. Test ==="
echo "  pgrok myapp 4000   # => https://myapp.${DOMAIN}"
