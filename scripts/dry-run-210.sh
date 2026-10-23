#!/usr/bin/env bash
# Dry runs of nog 2.1.0's unattended update (F-20 #56, F-22 #58, F-23 #59) on THIS desktop, with the
# freshly built binary (target/release/nog). Every case stops before anything changes: nobody at the
# keyboard, or sudo refused at once by a stand-in first on PATH (never SUDO_ASKPASS). The real
# updates happen in the KognogOS VM (scripts/vm-run-210.sh).
#
#   D1  update, nobody at the keyboard            → "Not updating", the cause said, exit 1
#   D2  update --yes, nobody, sudo wants a password → "Not updating", the password said, exit 1
#   D3  update --yes at a terminal, sudo refused   → the handoff answered "yes (--yes)", pacman's
#                                                    step fails and the run is cancelled: exit 1,
#                                                    "stopped" (nothing was updated, so not "incomplete";
#                                                    a LATER step failing is exit 3, proven in the VM, Y4)
#   D4  --help and the man page carry --yes and EXIT STATUS
#
#   scripts/dry-run-210.sh
#
# Logs to logs/dry-run-210-<time>.log (+ -latest link); the last line says OK.
set -uo pipefail
cd "$(dirname "$0")/.."
mkdir -p logs
log="logs/dry-run-210-$(date +%Y%m%d-%H%M%S).log"
ln -sfn "$(basename "$log")" logs/dry-run-210-latest.log
exec > >(tee "$log") 2>&1

NOG="$PWD/target/release/nog"
MAN="$PWD/nog.1"
export HOME="$(mktemp -d)"          # nog's own records stay out of the real home
mkdir -p "$HOME/stub"
printf '#!/bin/sh\necho "sudo: refused (a dry run: nothing may change)" >&2\nexit 1\n' > "$HOME/stub/sudo"
chmod 755 "$HOME/stub/sudo"
export PATH="$HOME/stub:$PATH"
trap 'rm -rf --one-file-system "$HOME"' EXIT
fail=0
expect() {  # $1 case, $2 wanted exit, $3 got exit, $4 file, $5.. phrases that must appear
  local id="$1" want="$2" got="$3" f="$4"; shift 4
  [ "$got" = "$want" ] || { echo "  $id: FAIL — exit $got, wanted $want"; fail=1; }
  for p in "$@"; do
    grep -qF -- "$p" "$f" || { echo "  $id: FAIL — missing: $p"; fail=1; }
  done
  echo "  $id: checked (exit $got)"
}
clean() { sed 's/\x1b\[[0-9;?]*[a-zA-Z]//g; s/\r//g' "$1"; }

"$NOG" --version

echo; echo "################ D1: nog update, nobody at the keyboard"
"$NOG" update </dev/null >"$HOME/d1" 2>&1; e=$?
clean "$HOME/d1" | grep -v '^$' | tail -n 12
expect D1 1 $e "$HOME/d1" "Not updating" "Nothing is at the keyboard" "nog update --yes"

echo; echo "################ D2: nog update --yes, nobody at the keyboard, sudo wants a password"
env -u NOG_ASKPASS "$NOG" update --yes </dev/null >"$HOME/d2" 2>&1; e=$?
clean "$HOME/d2" | grep -v '^$' | tail -n 12
expect D2 1 $e "$HOME/d2" "Not updating" "sudo still asks for a password"

echo; echo "################ D3: nog update --yes at a terminal, sudo refused"
script -qec "$NOG update --yes" /dev/null </dev/null >"$HOME/d3.raw" 2>&1
clean "$HOME/d3.raw" > "$HOME/d3"
e=$(tail -n 1 "$(ls -t "$HOME"/.local/share/nog/logs/*nog-runs.csv | head -1)" | cut -d, -f5)
grep -E '^(nog:|==> )' "$HOME/d3" | tail -n 14
echo "  runs line: $(tail -n 1 "$(ls -t "$HOME"/.local/share/nog/logs/*nog-runs.csv | head -1)")"
expect D3 1 "$e" "$HOME/d3" "Begin the handoff? yes (--yes)" "pacman exited with status 1" "sudo: refused"
grep -q ',stopped$' "$(ls -t "$HOME"/.local/share/nog/logs/*nog-runs.csv | head -1)" \
  && echo "  D3: runs line says stopped" || { echo "  D3: FAIL — runs line"; fail=1; }

echo; echo "################ D4: --help and the man page"
"$NOG" update --help >"$HOME/d4" 2>&1
MANWIDTH=100 man -l "$MAN" 2>/dev/null >>"$HOME/d4"
grep -E -- '--yes|EXIT STATUS|incomplete' "$HOME/d4" | head -n 8
expect D4 0 0 "$HOME/d4" "--yes" "EXIT STATUS" "3"

echo
[ $fail = 0 ] && echo "OK — every case stopped where it should; nothing changed." || echo "FAILED — read the lines above."
echo "Log: $log"
