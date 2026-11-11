#!/usr/bin/env bash
# Dry run of nog 2.1.1's reverse soname rule (F-30 #67) on THIS desktop, read only: the installed nog
# and the freshly built one (target/release/nog) each print `nog update --json`, which stops before
# any question, from the same copy of the hold record in a throwaway HOME. Nothing is installed.
#
#   R1  the OpenSSL 4 rebuilds (curl, git, krb5, libssh2, openssh, sudo, qca-qt6) are Ready with the
#       installed nog and Held, coupled to openssl, with the new one
#   R2  what else the new rule holds, listed by name, so the cost is seen, not guessed
#
#   scripts/dry-run-211.sh
#
# Logs to logs/dry-run-211-<time>.log (+ -latest link); the last line says OK.
set -uo pipefail
cd "$(dirname "$0")/.."
mkdir -p logs
log="logs/dry-run-211-$(date +%Y%m%d-%H%M%S).log"
ln -sfn "$(basename "$log")" logs/dry-run-211-latest.log
exec > >(tee "$log") 2>&1

NEW="$PWD/target/release/nog"
OLD="${NOG_OLD:-/usr/bin/nog}"
REAL_STATE="$HOME/.local/state/nog"
export HOME="$(mktemp -d)"          # nog's own records stay out of the real home
mkdir -p "$HOME/stub" "$HOME/.local/state"
printf '#!/bin/sh\necho "sudo: refused (a dry run: nothing may change)" >&2\nexit 1\n' > "$HOME/stub/sudo"
chmod 755 "$HOME/stub/sudo"
export PATH="$HOME/stub:$PATH"
trap 'rm -rf --one-file-system "$HOME"' EXIT

run() {  # $1 binary, $2 out file: a fresh copy of the real hold record each time
  rm -rf "$HOME/.local/state/nog"; cp -r "$REAL_STATE" "$HOME/.local/state/nog" 2>/dev/null
  "$1" update --json </dev/null >"$2" 2>"$2.err"
}
"$OLD" --version; "$NEW" --version
run "$OLD" "$HOME/old.json"; run "$NEW" "$HOME/new.json"

python3 -I - "$HOME/old.json" "$HOME/new.json" <<'PY'
import json, sys
old, new = (json.load(open(f)) for f in sys.argv[1:3])
seven = ["curl", "git", "krb5", "libssh2", "openssh", "sudo", "qca-qt6"]
def where(d, n):
    for b in ("ready", "held", "unknown"):
        for e in d[b]:
            if e["name"] == n: return b, e.get("coupled_to")
    return None, None
fail = 0
print("\n################ R1: the seven, installed nog -> new nog")
for n in seven:
    o, n2 = where(old, n), where(new, n)
    ok = n2[0] == "held" and n2[1] == "openssl"
    fail |= not ok
    print(f"  {n:10} {str(o[0]):6} -> {n2[0]} (coupled to {n2[1]})  {'ok' if ok else 'FAIL'}")
print("\n################ R2: everything the new rule moved from Ready to Held")
was_ready = {e["name"] for e in old["ready"]}
moved = sorted((e["name"], e.get("coupled_to"), e["days_remaining"]) for e in new["held"] if e["name"] in was_ready)
for m in moved: print(f"  {m[0]:28} coupled to {m[1]} ({m[2]} days)")
print(f"\n  ready {len(old['ready'])} -> {len(new['ready'])} · held {len(old['held'])} -> {len(new['held'])} · moved {len(moved)}")
back = sorted(e["name"] for e in new["ready"] if e["name"] not in was_ready)
if back: print("  FAIL: Ready now but not before:", back); fail = 1
print("\nOK" if not fail else "\nFAILED")
sys.exit(fail)
PY
