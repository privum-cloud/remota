#!/usr/bin/env bash
# Deploy / upgrade the Remota desktop app (.deb) to a LAN host over SSH.
#
# The host password is read from the SSHPASS env var (never hardcoded here) and is
# used both for the SSH login and for `sudo -S` on the target (same password on our
# boxes). Build the .deb first: `npm run tauri -- build --bundles deb`.
#
# Usage:
#   SSHPASS='<password>' ./scripts/deploy-app.sh sysadmin@192.168.1.241 [deb-path]
#
# Fleet (see central memory): ironman 192.168.1.241, groot 192.168.1.242 — run once per host.
set -euo pipefail

TARGET="${1:?usage: SSHPASS=<pw> ./scripts/deploy-app.sh user@host [deb-path]}"
DEB="${2:-src-tauri/target/release/bundle/deb/Remota_0.1.0_amd64.deb}"
: "${SSHPASS:?set SSHPASS to the target password}"

[[ -f "$DEB" ]] || { echo "deb not found: $DEB — build it: npm run tauri -- build --bundles deb" >&2; exit 1; }
command -v sshpass >/dev/null || { echo "sshpass not installed" >&2; exit 1; }

BASE="$(basename "$DEB")"
OPTS="-o StrictHostKeyChecking=accept-new -o PubkeyAuthentication=no -o ConnectTimeout=15"

echo ">> $TARGET: copying $BASE"
sshpass -e scp $OPTS "$DEB" "$TARGET:/tmp/$BASE"

echo ">> $TARGET: installing (apt resolves deps)"
# sshpass -e uses SSHPASS for the SSH auth; the here-string feeds the same password to sudo -S.
sshpass -e ssh $OPTS "$TARGET" \
	"sudo -S -p '' apt-get install -y /tmp/$BASE >/dev/null 2>&1; \
	 dpkg -l | grep -E '^ii +remota ' | awk '{print \"installed:\", \$2, \$3}'; \
	 rm -f /tmp/$BASE" <<<"$SSHPASS"

echo ">> $TARGET: done"
