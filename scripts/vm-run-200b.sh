#!/usr/bin/env bash
# The second half of the 2.0.0 VM run: the cases the first run's driver got
# wrong (it typed a literal "\n" and answered sudo's rule on nog's own words
# "no password"), rerun on the VM as the first run left it — cowsay installed,
# wonderpen removed (its runtime unused), hello-world with an old revision
# kept, five orphans, yay's snapd build tree. No revert: the garbage is the
# point. Logs to logs/vm-200b-<case>.log and logs/vm-run-200b-<time>.log.
set -uo pipefail
cd "$(dirname "$0")/.."
mkdir -p logs
log="logs/vm-run-200b-$(date +%Y%m%d-%H%M%S).log"
ln -sfn "$(basename "$log")" logs/vm-run-200b-latest.log
exec > >(tee "$log") 2>&1
VM=kognog-hypeforge
# The VM's password (test-only) lives outside the repository (2026-10-22): a private file.
VMPW="$(cat "${NOG_VM_PASSWORD_FILE:-$HOME/.config/nog/vm-password}" 2>/dev/null)"
[ -n "$VMPW" ] || { echo "no VM password: put it in ~/.config/nog/vm-password (or NOG_VM_PASSWORD_FILE)"; exit 1; }
X() { scripts/vm-exec.py --timeout "${T:-900}" -- "$@"; }
J() { scripts/vm-exec.py --timeout "${T:-900}" -- "runuser -u javier -- bash -lc '$1'"; }
D() {
  local id="$1" cmd="$2"; shift 2
  local rules=""
  for r in "$@"; do rules="$rules '$r'"; done
  echo; echo "################ $id: $cmd"
  scripts/vm-exec.py --timeout 900 -- "runuser -u javier -- env HOME=/home/javier vm-drive --timeout 800 'cd ~ && $cmd' '\\[sudo\\] password for' '$VMPW' $rules" | tee "logs/vm-200b-$id.log" | tail -30
}
echo "== VM: start as it is (no revert)"
virsh -c qemu:///system start "$VM" >/dev/null 2>&1 || true
for i in $(seq 1 60); do
  virsh -c qemu:///system qemu-agent-command "$VM" '{"execute":"guest-ping"}' >/dev/null 2>&1 && break
  sleep 5
done
sleep 10
X 'nog --version; pacman -Q cowsay; pacman -Qtdq | wc -l; snap list --all | grep -c disabled; ls /home/javier/.cache/yay/'

echo; echo "== V3a again: reinstall through pacman, y to its question"
D V3a "nog reinstall cowsay" 'Proceed with installation' 'y'

echo; echo "== V4c again: nog clean, all, the tools' questions answered"
D V4c "nog clean" 'Clean up which' 'all' 'Do you want to remove these packages' 'y' 'Proceed with these changes' 'y'
X 'echo "orphans: $(pacman -Qtdq | wc -l)"; echo "disabled snap revisions: $(snap list --all | grep -c disabled)"; echo "-- runtimes:"; flatpak list --runtime --columns=ref; echo "-- yay cache:"; ls -A /home/javier/.cache/yay/'
T=600 J 'nog clean --json 2>/dev/null' | python3 -I -c 'import json,sys
raw=[l for l in sys.stdin.read().splitlines() if l.startswith("{")]
d=json.loads(raw[-1]) if raw else {}
print("after:", [(c["id"], c["items"]) for c in d.get("categories",[])])' | tee logs/vm-200b-V4d.log

echo; echo "== V4e again: --only still asks; n keeps everything"
X 'snap remove hello-world >/dev/null 2>&1; snap install hello-world --revision=27 2>&1 | tail -1; snap refresh hello-world 2>&1 | tail -1'
D V4e "nog clean --only snap" 'Clean up snap' 'n'
X 'echo "disabled snap revisions: $(snap list --all | grep -c disabled)"'
D V4f "nog clean --only snap --yes"
X 'echo "disabled snap revisions: $(snap list --all | grep -c disabled)"'

echo; echo "== V5e again: history sees the runs (HOME right this time)"
D V5e "nog history --days 1"

echo; echo "== shutting the VM down"
virsh -c qemu:///system shutdown "$VM" >/dev/null 2>&1 || true
echo "OK: VM run (part two) done"
