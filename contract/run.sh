#!/usr/bin/env bash
# Contract test: do the JSON-RPC calls our routes make match what the REAL, pinned signal-cli accepts?
#   contract/run.sh [IMAGE]      IMAGE defaults to building ./Dockerfile as modern-signal-cli-api:contract
# 1. dumps the real surface (methods + exact param names) from the signal-cli inside the image
# 2. runs the test-suite with CONTRACT_CAPTURE, recording every method + param name our routes sent
# 3. compares them (contract/check.py); known, documented exceptions live in contract/known-gaps.txt
set -euo pipefail
cd "$(dirname "$0")/.."
IMAGE="${1:-}"
if [ -z "$IMAGE" ]; then IMAGE=modern-signal-cli-api:contract; docker build -q -t "$IMAGE" . >/dev/null; fi
WORK="$(mktemp -d)"; trap 'rm -rf "$WORK"' EXIT
docker run --rm --entrypoint bash -v "$PWD/contract:/c:ro" "$IMAGE" \
    -c 'cd /opt/signal-cli-* && java -cp "lib/*" /c/SpecDump.java' > "$WORK/spec.json"
CONTRACT_CAPTURE="$WORK/capture.jsonl" cargo test --locked --quiet >/dev/null
python3 -I contract/check.py "$WORK/spec.json" "$WORK/capture.jsonl" contract/known-gaps.txt
