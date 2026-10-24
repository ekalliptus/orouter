#!/usr/bin/env bash
# Cloudflare origin firewall lock for the ORouter VPS (GCP box).
#   lock : ufw 80/443 accept ONLY from Cloudflare ranges
#   sync : refresh the cloudflare-origin rules (weekly cron)
# The GCP firewall is the outer layer; this is the inner layer.
set -euo pipefail
MODE="${1:?usage: cf-firewall-sync.sh lock|sync}"

RULES_OUT=$(mktemp)
trap 'rm -f "$RULES_OUT"' EXIT

delete_cf_rules() {
	# Delete every ufw rule carrying the cloudflare-origin comment, highest
	# numbered first so indices stay valid while deleting.
	mapfile -t nums < <(ufw status numbered | grep 'cloudflare-origin' | grep -oE '^\[[ 0-9]+\]' | grep -oE '[0-9]+' | sort -rn)
	for n in "${nums[@]}"; do ufw --force delete "$n" >/dev/null 2>&1 || true; done
}

add_cf_rules() {
	curl -fsS https://www.cloudflare.com/ips-v4 > "$RULES_OUT"
	# ips-v4 response has no trailing newline — without this, the last v4
	# range merges with the first v6 line into one invalid entry.
	printf '\n' >> "$RULES_OUT"
	curl -fsS https://www.cloudflare.com/ips-v6 >> "$RULES_OUT"
	local n=0
	while IFS= read -r range; do
		[ -z "$range" ] && continue
		ufw allow from "$range" to any port 80 proto tcp comment 'cloudflare-origin' >/dev/null
		ufw allow from "$range" to any port 443 proto tcp comment 'cloudflare-origin' >/dev/null
		n=$((n + 2))
	done < "$RULES_OUT"
	echo "added $n cloudflare-origin rules"
}

case "$MODE" in
	lock)
		ufw delete allow 80/tcp >/dev/null 2>&1 || true
		ufw delete allow 443/tcp >/dev/null 2>&1 || true
		delete_cf_rules
		add_cf_rules
		;;
	sync)
		delete_cf_rules
		add_cf_rules
		;;
	*) echo "unknown mode: $MODE" >&2; exit 1 ;;
esac
ufw status | grep -c 'cloudflare-origin'
