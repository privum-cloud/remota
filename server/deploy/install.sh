#!/usr/bin/env bash
# Install/refresh remota-relay on a fresh Debian/Ubuntu rendezvous host.
#
# Usage:
#   ./install.sh                 # build the release binary here (needs cargo) and install
#   ./install.sh /path/to/remota-relay   # install a prebuilt binary you scp'd over
#
# Caddy (TLS) is installed/configured separately — see README.md and Caddyfile.
set -euo pipefail

HERE="$(cd "$(dirname "$0")" && pwd)"

# 1) Resolve the binary: explicit arg, else build the release from the workspace.
if [[ "${1:-}" != "" ]]; then
	BIN="$1"
elif command -v cargo >/dev/null 2>&1; then
	echo ">> building release binary (cargo build --release -p remota-relay)"
	( cd "$HERE/.." && cargo build --release -p remota-relay )
	BIN="$HERE/../target/release/remota-relay"
else
	echo "!! no binary arg and cargo not found. Build elsewhere and pass the path:" >&2
	echo "   ./install.sh /path/to/remota-relay" >&2
	exit 1
fi
[[ -x "$BIN" ]] || { echo "!! binary not found/executable: $BIN" >&2; exit 1; }

# 2) Dedicated unprivileged service user.
sudo useradd --system --no-create-home --shell /usr/sbin/nologin remota 2>/dev/null || true

# 3) Install binary + config dir.
sudo install -Dm755 "$BIN" /usr/local/bin/remota-relay
sudo install -d -m 750 -o remota -g remota /etc/remota

# 4) Generate the env file (with a fresh enrollment token) on first install only.
if ! sudo test -f /etc/remota/relay.env; then
	TOKEN="$(head -c 32 /dev/urandom | base64 | tr -d '/+=' | head -c 40)"
	printf 'REMOTA_RELAY_LISTEN=127.0.0.1:8787\nREMOTA_ENROLL_TOKEN=%s\n' "$TOKEN" \
		| sudo tee /etc/remota/relay.env >/dev/null
	sudo chown remota:remota /etc/remota/relay.env
	sudo chmod 640 /etc/remota/relay.env
	echo ">> generated /etc/remota/relay.env with a new enrollment token"
else
	# Existing file: the relay refuses to start on a placeholder/short token, so catch it
	# here instead of leaving a dead service behind a "success" message.
	EXISTING="$(sudo sed -n 's/^REMOTA_ENROLL_TOKEN=//p' /etc/remota/relay.env | tail -1)"
	if [[ -z "$EXISTING" || "$EXISTING" == "dev-enroll" || "$EXISTING" == CHANGE_ME* || ${#EXISTING} -lt 16 ]]; then
		echo "!! /etc/remota/relay.env has no usable REMOTA_ENROLL_TOKEN." >&2
		echo "   The relay will refuse to start. Set a long random secret:" >&2
		echo "   head -c 32 /dev/urandom | base64 | tr -d '/+=' | head -c 40" >&2
		exit 1
	fi
fi

# 5) systemd unit.
sudo install -Dm644 "$HERE/remota-relay.service" /etc/systemd/system/remota-relay.service
sudo systemctl daemon-reload
sudo systemctl enable --now remota-relay

echo
echo ">> remota-relay is up on 127.0.0.1:8787 (put Caddy in front for TLS — see Caddyfile)"
sudo systemctl --no-pager --full status remota-relay | head -n 6 || true
echo
echo ">> enrollment token (give agents this with --token):"
sudo grep REMOTA_ENROLL_TOKEN /etc/remota/relay.env
