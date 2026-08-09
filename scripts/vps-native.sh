#!/usr/bin/env bash
set -Eeuo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SERVICE="orouter.service"
SERVICE_FILE="/etc/systemd/system/$SERVICE"
ENV_DIR="/etc/orouter"
ENV_FILE="$ENV_DIR/orouter.env"
RUNTIME_USER="orouter"
RELEASES_DIR="/opt/orouter/releases"
CURRENT_LINK="/opt/orouter/current"
PREVIOUS_LINK="/opt/orouter/previous"
DATA_DIR="/var/lib/9router"
RUST_PORT="20130"
DEPLOY_STAGE=""

cleanup() {
  [[ -z ${DEPLOY_STAGE:-} ]] || rm -rf "$DEPLOY_STAGE"
}
trap cleanup EXIT

log() { printf '[orouter] %s\n' "$*"; }
warn() { printf '[orouter] warning: %s\n' "$*" >&2; }
die() { printf '[orouter] error: %s\n' "$*" >&2; exit 1; }

require_deploy_user() {
  [[ ${EUID:-$(id -u)} -ne 0 ]] || die "run as a normal deploy user with sudo access, not root"
  command -v sudo >/dev/null || die "sudo is required"
}

load_tool_paths() {
  export CARGO_HOME="${CARGO_HOME:-$HOME/.cargo}"
  export BUN_INSTALL="${BUN_INSTALL:-$HOME/.bun}"
  export PATH="$CARGO_HOME/bin:$BUN_INSTALL/bin:$PATH"
}

check_linux() {
  [[ -r /etc/os-release ]] || die "Ubuntu/Debian is required"
  # shellcheck disable=SC1091
  source /etc/os-release
  case "${ID:-} ${ID_LIKE:-}" in
    *ubuntu*|*debian*) ;;
    *) die "unsupported OS: ${PRETTY_NAME:-unknown}; use Ubuntu/Debian" ;;
  esac
}

install_rust() {
  load_tool_paths
  if ! command -v rustup >/dev/null; then
    log "installing rustup (stable, minimal profile)"
    curl --proto '=https' --tlsv1.2 -fsS https://sh.rustup.rs \
      | sh -s -- -y --profile minimal --default-toolchain stable
    load_tool_paths
  fi
  if ! rustup toolchain list | grep -q '^stable-'; then
    rustup toolchain install stable --profile minimal >/dev/null
  fi
  rustup component add rustfmt >/dev/null
}

install_bun() {
  load_tool_paths
  if ! command -v bun >/dev/null; then
    log "installing Bun"
    curl -fsSL https://bun.sh/install | bash
    load_tool_paths
  fi
}

rust_check() {
  install_rust
  rustc --version
  cargo --version
  rustup check || warn "rustup check failed (network unavailable?); continuing with installed toolchain"
}

rust_update() {
  install_rust
  log "updating stable Rust toolchain"
  rustup update stable
  rustup component add rustfmt
  rustc --version
}

bootstrap() {
  require_deploy_user
  check_linux
  log "installing VPS build/runtime prerequisites"
  sudo apt-get update
  sudo DEBIAN_FRONTEND=noninteractive apt-get install -y \
    build-essential ca-certificates curl openssl pkg-config sqlite3 unzip

  install_rust
  install_bun

  if ! getent passwd "$RUNTIME_USER" >/dev/null; then
    sudo useradd --system --home-dir "$DATA_DIR" --create-home \
      --shell /usr/sbin/nologin "$RUNTIME_USER"
  fi
  sudo install -d -o root -g root -m 0755 "$RELEASES_DIR"
  sudo install -d -o "$RUNTIME_USER" -g "$RUNTIME_USER" -m 0750 "$DATA_DIR" "$DATA_DIR/backups"
  sudo install -d -o root -g "$RUNTIME_USER" -m 0750 "$ENV_DIR"

  if [[ ! -f "$ENV_FILE" ]]; then
    local jwt password api_secret
    jwt="$(openssl rand -hex 32)"
    password="$(openssl rand -hex 12)"
    api_secret="$(openssl rand -hex 32)"
    sudo tee "$ENV_FILE" >/dev/null <<EOF
DATA_DIR=$DATA_DIR
STATIC_DIR=$CURRENT_LINK/web
RUST_HOST=127.0.0.1
RUST_PORT=$RUST_PORT
RUST_LOG_LEVEL=info
RUST_BODY_MAX_MB=128
RUST_SHUTDOWN_TIMEOUT=15s
JWT_SECRET=$jwt
INITIAL_PASSWORD=$password
API_KEY_SECRET=$api_secret
AUTH_COOKIE_SECURE=true
EOF
    sudo chown root:"$RUNTIME_USER" "$ENV_FILE"
    sudo chmod 0640 "$ENV_FILE"
    printf '\n[orouter] initial dashboard password (save now): %s\n\n' "$password"
  else
    log "preserving existing $ENV_FILE"
  fi

  sudo install -m 0644 "$ROOT/deploy/orouter.service" "$SERVICE_FILE"
  sudo systemctl daemon-reload
  sudo systemctl enable "$SERVICE"

  if ! command -v caddy >/dev/null; then
    warn "Caddy is not installed. Install it, then copy/edit deploy/Caddyfile.example."
  fi

  log "bootstrap complete"
  log "next: ./scripts/vps-native.sh deploy"
}

build_release() {
  load_tool_paths
  install_rust
  install_bun

  if [[ ${UPDATE_RUST:-0} == 1 ]]; then
    rust_update
  else
    rust_check
  fi

  git -C "$ROOT" diff --quiet || die "tracked files are dirty; commit or restore them before deploy"
  log "testing and building Rust"
  cargo fmt --manifest-path "$ROOT/rust-backend/Cargo.toml" -- --check
  cargo test --manifest-path "$ROOT/rust-backend/Cargo.toml" --locked
  cargo build --manifest-path "$ROOT/rust-backend/Cargo.toml" --release --locked

  log "building React"
  (
    cd "$ROOT/react-web"
    bun install --frozen-lockfile
    bun run build
  )
}

backup_database() {
  local db="$DATA_DIR/db/data.sqlite"
  [[ -f "$db" ]] || return 0
  local backup="$DATA_DIR/backups/data-$(date -u +%Y%m%dT%H%M%SZ).sqlite"
  log "backing up SQLite to $backup"
  sudo -u "$RUNTIME_USER" sqlite3 "$db" ".backup '$backup'"
}

switch_current() {
  local target="$1"
  local next="${CURRENT_LINK}.next"
  sudo rm -f "$next"
  sudo ln -s "$target" "$next"
  sudo mv -Tf "$next" "$CURRENT_LINK"
}

wait_for_health() {
  local port
  port="$(sudo grep -E '^RUST_PORT=' "$ENV_FILE" 2>/dev/null | cut -d= -f2 || true)"
  port="${port:-$RUST_PORT}"
  local url="http://127.0.0.1:$port/health"
  for _ in $(seq 1 30); do
    if curl -fsS --max-time 2 "$url" >/dev/null; then
      return 0
    fi
    sleep 1
  done
  return 1
}

rollback_to() {
  local target="$1"
  [[ -n "$target" && -d "$target" ]] || die "rollback target is missing: $target"
  switch_current "$target"
  sudo systemctl restart "$SERVICE"
  wait_for_health || die "rollback target failed health check: $target"
}

deploy() {
  require_deploy_user
  check_linux
  [[ -f "$SERVICE_FILE" && -f "$ENV_FILE" ]] || die "run bootstrap first"
  build_release
  backup_database

  local sha release previous stage
  sha="$(git -C "$ROOT" rev-parse --short=12 HEAD)"
  release="$RELEASES_DIR/${sha}-$(date -u +%Y%m%dT%H%M%SZ)"
  previous="$(readlink -f "$CURRENT_LINK" 2>/dev/null || true)"
  stage="$(mktemp -d)"
  DEPLOY_STAGE="$stage"

  install -m 0755 "$ROOT/rust-backend/target/release/orouter-backend" "$stage/orouter-backend"
  mkdir -p "$stage/web"
  cp -a "$ROOT/react-web/dist/." "$stage/web/"

  log "installing release $release"
  sudo install -d -o root -g root -m 0755 "$release" "$release/web"
  sudo install -m 0755 "$stage/orouter-backend" "$release/orouter-backend"
  sudo cp -a "$stage/web/." "$release/web/"
  sudo chown -R root:root "$release"

  if [[ -n "$previous" && -d "$previous" ]]; then
    sudo ln -sfn "$previous" "$PREVIOUS_LINK"
  fi
  switch_current "$release"

  sudo systemctl restart "$SERVICE"
  if ! wait_for_health; then
    sudo journalctl -u "$SERVICE" -n 40 --no-pager >&2 || true
    if [[ -n "$previous" && -d "$previous" ]]; then
      warn "new release failed health check; rolling back"
      rollback_to "$previous"
    fi
    die "deployment failed"
  fi

  log "deployed $sha"
  status
}

rollback() {
  require_deploy_user
  local previous current
  previous="$(readlink -f "$PREVIOUS_LINK" 2>/dev/null || true)"
  current="$(readlink -f "$CURRENT_LINK" 2>/dev/null || true)"
  [[ -n "$previous" && -d "$previous" ]] || die "no previous release recorded"
  rollback_to "$previous"
  if [[ -n "$current" && -d "$current" ]]; then
    sudo ln -sfn "$current" "$PREVIOUS_LINK"
  fi
  log "rolled back to $previous"
}

status() {
  local port
  port="$(sudo grep -E '^RUST_PORT=' "$ENV_FILE" 2>/dev/null | cut -d= -f2 || true)"
  port="${port:-$RUST_PORT}"
  sudo systemctl --no-pager --full status "$SERVICE" || true
  printf '\nCurrent release: %s\n' "$(readlink -f "$CURRENT_LINK" 2>/dev/null || echo none)"
  printf 'Health: '
  curl -fsS --max-time 2 "http://127.0.0.1:$port/health" || printf 'unavailable'
  printf '\n'
}

logs() {
  exec sudo journalctl -u "$SERVICE" -f -n 100
}

usage() {
  cat <<'EOF'
Usage: ./scripts/vps-native.sh COMMAND

Commands:
  bootstrap    Install prerequisites, Rust/Bun, service user and systemd unit
  deploy       Test, build, backup DB, atomically release, health-check, rollback on failure
  rollback     Switch to the previously deployed release
  status       Show systemd status, current release and health
  logs         Follow journald logs
  rust-check   Show installed Rust and available updates
  rust-update  Update stable Rust explicitly

Update Rust during deploy:
  UPDATE_RUST=1 ./scripts/vps-native.sh deploy
EOF
}

load_tool_paths
case "${1:-}" in
  bootstrap) bootstrap ;;
  deploy) deploy ;;
  rollback) rollback ;;
  status) status ;;
  logs) logs ;;
  rust-check) rust_check ;;
  rust-update) rust_update ;;
  *) usage; exit 1 ;;
esac
