#!/usr/bin/env bash
# Dry runs of nog 1.9.0's install chain, remove and keep on THIS desktop,
# with the freshly built binary (target/release/nog). Every case stops before
# anything changes: a name found nowhere, nobody at the keyboard, "n" to the
# question, or sudo refused (a stand-in sudo first on PATH, which refuses
# at once instead of waiting). Nothing is installed or removed.
#
#   scripts/dry-run-190.sh
#
# Logs to logs/dry-run-190-<time>.log (+ -latest link); the last line says OK.
set -uo pipefail
cd "$(dirname "$0")/.."
mkdir -p logs
log="logs/dry-run-190-$(date +%Y%m%d-%H%M%S).log"
ln -sfn "$(basename "$log")" logs/dry-run-190-latest.log
exec > >(tee "$log") 2>&1

NOG="$PWD/target/release/nog"
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

# case <id> <stdin-or-> <nog args...>: "-" means no terminal at all (a script's
# shell); anything else is typed into a real terminal (script(1) gives one).
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

"$NOG" --version
echo "home: $HOME"

# ── install: the chain ──────────────────────────────────────────────────────
case_ A1  -        install zzz-nope
case_ A2  -        install wonderpen
case_ A3  'n\n'    install hello-world
case_ A4  -        install gimp
case_ A5  -        install flatpak/org.gimp.GIMP
case_ A6  -        install calculator
case_ A6b -        install --yes calculator
case_ A6c 'n\n'    install calculator
case_ A6d '9\nx\n\n' install calculator
case_ A6e -        install flatpak/calculator
case_ A7  'n\n'    install snap/hello
case_ A8  -        install --yes aur/neofetch
case_ A9  -        install --yes cowsay
case_ A10 'n\n'    install cowsay flatpak/wonderpen snap/hello-world
case_ A11 -        install snap/zzz-nope
case_ A12 -        install flatpak/zzz-nope
case_ A13 -        install ./dist-rc/nog-1.8.0rc1-1-x86_64.pkg.tar.zst
# ── remove: packages, flatpaks, snaps; F-13 ─────────────────────────────────
case_ B1  -        remove zzz-nope
case_ B2  'n\n'    remove flatseal
case_ B3  'n\n'    remove hello
case_ B3b '\n'     remove hello
case_ B4  'n\n'    remove iwd
case_ B5  -        remove --yes iwd
case_ B6  -        remove iwd
# ── keep ────────────────────────────────────────────────────────────────────
case_ C1  -        keep zzz-nope
case_ C2  -        keep vlc-plugin-ffmpeg
echo
echo OK
