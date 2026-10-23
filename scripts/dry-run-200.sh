#!/usr/bin/env bash
# Dry runs of nog 2.0.0's command surface (C4), maintenance (C5) and the
# locks on THIS desktop, with the freshly built binary (target/release/nog).
# Every case stops before anything changes: a read-only command, a name found
# nowhere, nobody at the keyboard, "n" to the question, or sudo's password
# window refused (a stand-in sudo that refuses). Nothing is installed, removed,
# locked or cleaned. The 1.9.0 chain cases live in dry-run-190.sh.
#
#   scripts/dry-run-200.sh
#
# Logs to logs/dry-run-200-<time>.log (+ -latest link); the last line says OK.
set -uo pipefail
cd "$(dirname "$0")/.."
mkdir -p logs
log="logs/dry-run-200-$(date +%Y%m%d-%H%M%S).log"
ln -sfn "$(basename "$log")" logs/dry-run-200-latest.log
exec > >(tee "$log") 2>&1

NOG="$PWD/target/release/nog"
REAL_HOME="$HOME"
export HOME="$(mktemp -d)"          # nog's own records stay out of the real home
export NOG_ASKPASS=1
# sudo refused at once by a stand-in first on PATH (2026-10-22: the working rule says never set
# SUDO_ASKPASS). nog and the AUR helper both call `sudo` by name, so both meet it.
mkdir -p "$HOME/stub"
printf '#!/bin/sh\necho "sudo: refused (a dry run: nothing may change)" >&2\nexit 1\n' > "$HOME/stub/sudo"
chmod 755 "$HOME/stub/sudo"
export PATH="$HOME/stub:$PATH"
export NOG_EVENTS="$HOME/events.jsonl"
trap 'rm -rf --one-file-system "$HOME"' EXIT

case_() {
  local id="$1" input="$2"; shift 2
  echo; echo "################ $id: nog $*"
  : > "$NOG_EVENTS"
  if [ "$input" = "-" ]; then
    "$NOG" "$@" < /dev/null
  else
    printf '%b' "$input" | script -qec "$NOG $*" /dev/null
  fi
  echo "---- exit $? · events: $(tr '\n' ' ' < "$NOG_EVENTS")"
}
json_() {   # a JSON run: one document on stdout, nothing else; summarised
  local id="$1"; shift
  echo; echo "################ $id: nog $* (json)"
  "$NOG" "$@" < /dev/null | python3 -I -c '
import json,sys
raw=sys.stdin.read().strip().splitlines()
d=json.loads(raw[-1]); print("documents:", len(raw), "· kind:", d.get("kind"), "· keys:", sorted(k for k in d.keys()))
for k in ("results","packages","categories","runs","held","ready"):
    if k in d: print(f"  {k}: {len(d[k])}")
if "sources" in d: print("  sources:", d["sources"])
'
  echo "---- exit ${PIPESTATUS[0]}"
}

"$NOG" --version
echo "home: $HOME (logs read from $REAL_HOME for history/status)"

# ── search, list, info, tier: read-only ─────────────────────────────────────
case_ S1  -        search calc
json_ S1j          search calc --json
case_ S2  -        search zzz-nope-nothing
case_ L1  -        list
json_ L1j          list --json
case_ I1  -        info cowsay
case_ I2  -        info glibc
case_ I3  -        info flatseal
case_ I4  -        info hello
case_ I5  -        info zzz-nope
json_ I1j          info cowsay --json
case_ T1  -        tier glibc
case_ T2  -        tier lib32-glibc
case_ T3  -        tier cowsay
json_ T1j          tier linux-zen-headers --json
case_ T4  -        tier bash --set 2            # sudo refused → Not changed
case_ T5  -        tier bash --set 9            # not a tier
case_ T6  -        tier bash --set 3            # already the default → nothing changed
# ── lock / unlock ───────────────────────────────────────────────────────────
case_ K1  -        lock
json_ K1j          lock --json
case_ K2  -        lock zzz-nope                # not installed → stops
case_ K3  -        lock bash                    # sudo refused → Not locked
case_ K4  -        unlock bash                  # not locked → nothing to undo
# ── reinstall ───────────────────────────────────────────────────────────────
case_ R1  -        reinstall zzz-nope
case_ R2  -        reinstall hello              # a snap: refused with the way out
case_ R3  -        reinstall bash               # nobody at the keyboard
case_ R4  'y\n'    reinstall bash               # sudo refused → pacman stopped
case_ R5  -        reinstall --yes bash         # sudo refused → pacman stopped
# ── status / history ────────────────────────────────────────────────────────
HOME="$REAL_HOME" case_ H1 - history --days 2
HOME="$REAL_HOME" json_ H1j history --days 2 --json
HOME="$REAL_HOME" case_ H2 - status
HOME="$REAL_HOME" json_ H2j status --json
# ── clean (C5): the report, every refusal, every answer that changes nothing ─
json_ C1j          clean --json
case_ C1  -        clean                        # nobody to pick
case_ C2  'n\n'    clean
case_ C3  '\n'     clean                        # Enter = none
case_ C4  '9\nx\n\n' clean                      # asked again twice, then none
case_ C5  -        clean --only typo
case_ C6  -        clean --only snap            # nobody to confirm
case_ C7  'n\n'    clean --only snap,orphans
case_ C8  -        clean --only aur             # nothing in the temp home's cache
echo
echo "OK: dry runs done, nothing changed"
