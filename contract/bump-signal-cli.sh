#!/usr/bin/env bash
# Point the Dockerfile at a signal-cli release: version AND the SHA-256 of its tarball (a bump needs both).
#   contract/bump-signal-cli.sh            latest official release
#   contract/bump-signal-cli.sh 0.14.10    a specific one (with or without a leading v)
# Prints `up-to-date X` or `bumped X -> Y` and edits ./Dockerfile; run contract/run.sh afterwards.
set -euo pipefail
cd "$(dirname "$0")/.."
want="${1:-}"
if [ -z "$want" ]; then
    auth=()
    [ -n "${GITHUB_TOKEN:-}" ] && auth=(-H "Authorization: Bearer $GITHUB_TOKEN")
    want="$(curl -fsSL "${auth[@]}" https://api.github.com/repos/AsamK/signal-cli/releases/latest \
            | python3 -I -c 'import json,sys; print(json.load(sys.stdin)["tag_name"])')"
fi
want="${want#v}"
have="$(sed -n 's/^ARG SIGNAL_CLI_VERSION=//p' Dockerfile)"
if [ "$want" = "$have" ]; then echo "up-to-date $have"; exit 0; fi
tmp="$(mktemp)"; trap 'rm -f "$tmp"' EXIT
curl -fsSL -o "$tmp" "https://github.com/AsamK/signal-cli/releases/download/v${want}/signal-cli-${want}.tar.gz"
sum="$(sha256sum "$tmp" | cut -d' ' -f1)"
sed -i "s/^ARG SIGNAL_CLI_VERSION=.*/ARG SIGNAL_CLI_VERSION=${want}/; s/^ARG SIGNAL_CLI_SHA256=.*/ARG SIGNAL_CLI_SHA256=${sum}/" Dockerfile
echo "bumped ${have} -> ${want}"
