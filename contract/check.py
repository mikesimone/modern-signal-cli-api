#!/usr/bin/env python3
"""Contract check: what our routes send vs what the real signal-cli accepts.

  check.py SPEC.json CAPTURE.jsonl [KNOWN_GAPS.txt]

SPEC.json    from SpecDump.java run against the pinned signal-cli (the real surface)
CAPTURE.jsonl one {"method", "params": [names]} per request the test mock received
KNOWN_GAPS   lines of `method` or `method:param`, each preceded by a `# reason` comment

Fails (exit 1) for any call to a method signal-cli does not have, or any param name it would not read
(local commands silently ignore unknown params, which is the dangerous case), unless listed as a known gap.
Also fails on known-gap entries that no longer occur, so the list cannot rot."""
import json
import sys

MOCK_ONLY = {"simulateError"}  # methods the test mock invents


def load_gaps(path):
    gaps = set()
    if path:
        for line in open(path):
            line = line.split("#", 1)[0].strip()
            if line:
                gaps.add(line)
    return gaps


def main():
    spec = json.load(open(sys.argv[1]))
    calls = {}
    for line in open(sys.argv[2]):
        c = json.loads(line)
        calls.setdefault(c["method"], set()).update(c["params"])
    gaps = load_gaps(sys.argv[3] if len(sys.argv) > 3 else None)
    used_gaps, problems = set(), []

    for method, keys in sorted(calls.items()):
        if method in MOCK_ONLY:
            continue
        if method not in spec:
            if method in gaps:
                used_gaps.add(method)
            else:
                problems.append(f"{method}: signal-cli has no such JSON-RPC method")
            continue
        info = spec[method]
        allowed = set(info["params"])
        if info["account"]:
            allowed.add("account")
        for k in sorted(keys - allowed):
            g = f"{method}:{k}"
            if g in gaps:
                used_gaps.add(g)
            else:
                kind = "rejected" if info["kind"] == "typed" else "ignored silently"
                problems.append(f"{method}: param '{k}' is {kind} by signal-cli (accepts: {', '.join(sorted(allowed)) or 'none'})")

    stale = sorted(gaps - used_gaps)
    for g in stale:
        problems.append(f"known-gaps entry '{g}' no longer occurs - remove it")

    print(f"contract: {len(calls)} methods exercised against {len(spec)} real signal-cli methods; "
          f"{len(used_gaps)} documented known gaps; {len(problems)} problems")
    for p in problems:
        print("  FAIL", p)
    return 1 if problems else 0


sys.exit(main())
