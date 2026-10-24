#!/usr/bin/env bash
# Bootstrap a fresh Ubuntu VPS and deploy ORouter end-to-end.
# Run from the repo root on the LOCAL machine (Git Bash):
#   ./deploy/setup-new-vps.sh <ssh-target> [domain]
#   <ssh-target>  ssh destination, e.g. root@1.2.3.4 or an ~/.ssh/config alias
#   [domain]      optional site name for Caddy (e.g. router.ekalliptus.com).
#                 Omit while DNS does not point at the box yet — Caddy then
#                 serves plain HTTP :80 and is switched later.
set -euo pipefail

TARGET="${1:?usage: setup-new-vps.sh <ssh-target> [domain]}"
DOMAIN="${2:-}"

run() { ssh "$TARGET" "$@"; }

echo "== [1/7] base packages + Node 22 =="
run 'export DEBIAN_FRONTEND=noninteractive; sudo apt-get update -qq && sudo apt-get install -y -qq curl git build-essential python3-minimal ufw ca-certificates >/dev/null'
run 'if ! command -v node >/dev/null || [ "$(node -v | cut -dv -f2 | cut -d. -f1)" -lt 22 ]; then curl -fsSL https://deb.nodesource.com/setup_22.x | sudo -E bash - >/dev/null && sudo apt-get install -y -qq nodejs; fi; node -v'

echo "== [1b/7] swap 2GB (next build butuh lebih dari RAM kecil) =="
run 'if [ ! -f /swapfile ]; then sudo fallocate -l 2G /swapfile && sudo chmod 600 /swapfile && sudo mkswap /swapfile >/dev/null && sudo swapon /swapfile && echo "/swapfile none swap sw 0 0" | sudo tee -a /etc/fstab >/dev/null; fi; free -m | grep -i swap'

echo "== [2/7] firewall (22/80/443) =="
run 'sudo ufw allow OpenSSH >/dev/null 2>&1 || true; sudo ufw allow 80/tcp >/dev/null 2>&1 || true; sudo ufw allow 443/tcp >/dev/null 2>&1 || true; sudo ufw --force enable >/dev/null; sudo ufw status | head -5'

echo "== [3/7] sync source tree -> /opt/orouter =="
run 'sudo mkdir -p /opt/orouter && sudo chown $(whoami): /opt/orouter'
tar czf - \
  --exclude=./node_modules --exclude=./.git --exclude=./.next \
  --exclude=./.next-previous-build --exclude=./vue-web --exclude=./react-web \
  --exclude=./rust-backend --exclude=./tests --exclude=./docs --exclude=./gitbook \
  --exclude=./images --exclude=./.zcode --exclude=./tmp-final55.cjs \
  . | run 'tar xzf - -C /opt/orouter'
run 'cd /opt/orouter && ls package.json custom-server.js >/dev/null && echo source-ok'

echo "== [4/7] install deps + build =="
run 'cd /opt/orouter && npm ci --no-audit --no-fund 2>&1 | tail -1'
run 'cd /opt/orouter && NODE_OPTIONS=--max-old-space-size=1536 npm run build 2>&1 | tail -3'

echo "== [5/7] systemd service =="
scp deploy/templates/orouter.service "$TARGET:/tmp/orouter.service"
run 'sudo mv /tmp/orouter.service /etc/systemd/system/orouter.service && sudo systemctl daemon-reload && sudo systemctl enable --now orouter'
sleep 8
run 'systemctl is-active orouter && curl -s -m 8 http://127.0.0.1:20128/api/version'

echo "== [6/7] Caddy =="
run 'if ! command -v caddy >/dev/null; then sudo apt-get install -y -qq debian-keyring debian-archive-keyring apt-transport-https >/dev/null; curl -1sLf "https://dl.cloudsmith.io/public/caddy/stable/gpg.key" | sudo gpg --batch --yes --dearmor -o /usr/share/keyrings/caddy-stable-archive-keyring.gpg 2>/dev/null; curl -1sLf "https://dl.cloudsmith.io/public/caddy/stable/debian.deb.txt" | sudo tee /etc/apt/sources.list.d/caddy-stable.list >/dev/null; sudo apt-get update -qq && sudo apt-get install -y -qq caddy; fi'
SITE=":80"
[ -n "$DOMAIN" ] && SITE="$DOMAIN"
sed "s/__DOMAIN__/${SITE}/" deploy/templates/Caddyfile > /tmp/Caddyfile.rendered
scp /tmp/Caddyfile.rendered "$TARGET:/tmp/Caddyfile"
run 'sudo mv /tmp/Caddyfile /etc/caddy/Caddyfile && sudo systemctl enable --now caddy && sudo systemctl reload caddy 2>/dev/null || sudo systemctl restart caddy'

echo "== [7/7] verify =="
run 'curl -s -m 8 http://127.0.0.1:20128/api/version && systemctl is-active orouter caddy'
echo "done. next steps:"
echo "  - point DNS A record at this box, then re-run with the domain to switch Caddy off :80"
echo "  - Cloudflare orange cloud + SSL strict + rate-limit, then cf-firewall-sync lock"
