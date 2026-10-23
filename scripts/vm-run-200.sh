#!/usr/bin/env bash
# nog 2.0.0's real-change cases in the KognogOS VM (kognog-hypeforge), from
# the `nog-190-tested` snapshot: flatpak and snapd set up, yay present, nog
# 1.9.0 installed — the state the 1.9.0 run left. The fresh binary replaces
# nog, then: a lock seen by the update plan and undone; a tier moved and
# moved back; reinstalls through pacman and flatpak, the snap refusal; garbage
# made on purpose in every source and `nog clean` taking it all; the read-only
# commands for exit codes. Everything runs as the VM's user `javier` through
# vm-drive (answers typed when the prompts appear; the VM-only password).
#
#   scripts/vm-run-200.sh
#
# Logs to logs/vm-200-<case>.log and logs/vm-run-200-<time>.log (+ -latest).
set -uo pipefail
cd "$(dirname "$0")/.."
mkdir -p logs
log="logs/vm-run-200-$(date +%Y%m%d-%H%M%S).log"
ln -sfn "$(basename "$log")" logs/vm-run-200-latest.log
exec > >(tee "$log") 2>&1
VM=kognog-hypeforge
# The VM's password (test-only) lives outside the repository (2026-10-22): a private file.
VMPW="$(cat "${NOG_VM_PASSWORD_FILE:-$HOME/.config/nog/vm-password}" 2>/dev/null)"
[ -n "$VMPW" ] || { echo "no VM password: put it in ~/.config/nog/vm-password (or NOG_VM_PASSWORD_FILE)"; exit 1; }
X() { scripts/vm-exec.py --timeout "${T:-900}" -- "$@"; }
J() { scripts/vm-exec.py --timeout "${T:-900}" -- "runuser -u javier -- bash -lc '$1'"; }
# D <case> '<nog command>' [REGEX ANSWER ...]: drive a nog run as javier, log it
D() {
  local id="$1" cmd="$2"; shift 2
  local rules=""
  for r in "$@"; do rules="$rules '$r'"; done
  echo; echo "################ $id: $cmd"
  # vm-drive adds the newline itself; sudo's prompt is matched exactly (nog's
  # own words say "no password" and must not answer anything); HOME is set
  # because `runuser -u` keeps root's (the run logs must land in javier's).
  scripts/vm-exec.py --timeout 900 -- "runuser -u javier -- env HOME=/home/javier vm-drive --timeout 800 'cd ~ && $cmd' '\\[sudo\\] password for' '$VMPW' $rules" | tee "logs/vm-200-$id.log" | tail -25
}

echo "== VM: revert to nog-190-tested and start"
virsh -c qemu:///system destroy "$VM" >/dev/null 2>&1 || true
virsh -c qemu:///system snapshot-revert "$VM" nog-190-tested || exit 1
virsh -c qemu:///system start "$VM" >/dev/null 2>&1 || true
for i in $(seq 1 60); do
  virsh -c qemu:///system qemu-agent-command "$VM" '{"execute":"guest-ping"}' >/dev/null 2>&1 && break
  sleep 5
done
sleep 10
echo "== the fresh binary and the driver"
scripts/vm-put.py target/release/nog /tmp/nog.new
scripts/vm-put.py --mode 755 scripts/vm-drive.py /usr/local/bin/vm-drive
X 'mv -f /tmp/nog.new /usr/bin/nog; chmod 755 /usr/bin/nog; nog --version; id javier; snap version | head -2; flatpak --version; yay --version | head -1'

echo; echo "== V0: the read-only commands run (exit codes)"
for c in "nog status" "nog history --days 7" "nog lock" "nog tier glibc" "nog info bash" "nog search hello --json | head -c 300" "nog list --json | head -c 200"; do
  echo "---- $c"; T=600 J "$c 2>&1 | tail -6; echo exit=\${PIPESTATUS[0]}"
done

echo; echo "== V1: a lock, seen by the update plan, then undone"
READY=$(T=600 J 'nog update --json 2>/dev/null' | python3 -I -c 'import json,sys
raw=[l for l in sys.stdin.read().splitlines() if l.startswith("{")]
d=json.loads(raw[-1]) if raw else {}
r=d.get("ready",[]); print(r[0]["name"] if r else "")' 2>/dev/null)
echo "ready package to lock: '${READY:-none}'"
TARGET="${READY:-bash}"
D V1a "nog lock $TARGET"
X 'cat /etc/nog/locks.toml'
D V1b "nog lock"
T=600 J 'nog update --json 2>/dev/null' | python3 -I -c 'import json,sys
raw=[l for l in sys.stdin.read().splitlines() if l.startswith("{")]
d=json.loads(raw[-1]) if raw else {}
held=[h for h in d.get("held",[]) if h.get("locked")]
print("locked in the plan:", [(h["name"], h["note"]) for h in held])
print("ready:", len(d.get("ready",[])), "held:", len(d.get("held",[])))' | tee logs/vm-200-V1c.log
D V1d "nog unlock $TARGET"
X 'cat /etc/nog/locks.toml; echo ----; nog update --json 2>/dev/null | head -c 0'
D V1e "nog lock zzz-nope"

echo; echo "== V2: a tier moved and moved back"
D V2a "nog install --yes cowsay"
D V2b "nog tier cowsay --set 2"
D V2c "nog tier cowsay"
X 'grep -n cowsay /etc/nog/tier-pins.toml'
D V2d "nog tier cowsay --set 3"
D V2e "nog tier cowsay"

echo; echo "== V3: reinstalls through pacman and flatpak; the snap refusal"
D V3a "nog reinstall cowsay" 'Proceed with installation' 'y'
D V3b "nog install --yes flatpak/com.tominlab.wonderpen"
D V3c "nog reinstall --yes wonderpen"
D V3d "nog install --yes snap/hello-world"
D V3e "nog reinstall hello-world"
D V3f "nog reinstall zzz-nope"

echo; echo "== V4: garbage in every source, then nog clean takes it"
X 'pacman -S --asdeps --noconfirm sl >/dev/null 2>&1; pacman -Qtdq; echo "-- orphans above"'
D V4a "nog remove --yes wonderpen"
X 'snap remove hello-world >/dev/null 2>&1; snap install hello-world --revision=27 2>&1 | tail -1; snap refresh hello-world 2>&1 | tail -1; snap list --all | grep hello'
X 'ls /home/javier/.cache/yay/ 2>/dev/null | head; du -sh /home/javier/.cache/yay 2>/dev/null'
T=600 J 'nog clean --json 2>/dev/null' | python3 -I -c 'import json,sys
raw=[l for l in sys.stdin.read().splitlines() if l.startswith("{")]
d=json.loads(raw[-1]) if raw else {}
for c in d.get("categories",[]): print(c["id"], c["items"], round(c["bytes"]/1e6,1), "MB", c["names"][:4])' | tee logs/vm-200-V4b.log
D V4c "nog clean" 'Clean up which' 'all' 'Do you want to remove these packages' 'y' 'Proceed with these changes' 'y'
X 'echo "orphans: $(pacman -Qtdq | wc -l)"; snap list --all | grep -c disabled; echo "-- disabled snap revisions above"; flatpak list --runtime --columns=ref; ls -A /home/javier/.cache/yay/ 2>/dev/null; echo "-- yay cache above"'
T=600 J 'nog clean --json 2>/dev/null' | python3 -I -c 'import json,sys
raw=[l for l in sys.stdin.read().splitlines() if l.startswith("{")]
d=json.loads(raw[-1]) if raw else {}
print("after:", [(c["id"], c["items"]) for c in d.get("categories",[])])' | tee logs/vm-200-V4d.log
D V4e "nog clean --only snap" 'Clean up snap' 'n'

echo; echo "== V5: info/search/list see every source"
D V5a "nog info cowsay"
D V5b "nog info hello-world"
T=600 J 'nog search hello --json 2>/dev/null' | python3 -I -c 'import json,sys,collections
raw=[l for l in sys.stdin.read().splitlines() if l.startswith("{")]
d=json.loads(raw[-1]) if raw else {}
print(dict(collections.Counter(r["source"] if r["source"] in ("aur","flatpak","snap") else "repo" for r in d.get("results",[]))), d.get("sources"))' | tee logs/vm-200-V5c.log
T=600 J 'nog list --json 2>/dev/null' | python3 -I -c 'import json,sys,collections
raw=[l for l in sys.stdin.read().splitlines() if l.startswith("{")]
d=json.loads(raw[-1]) if raw else {}
print(len(d.get("packages",[])), dict(collections.Counter(p["source"] for p in d.get("packages",[]) if p["source"] in ("flatpak","snap"))))' | tee logs/vm-200-V5d.log
D V5e "nog history --days 1"

echo; echo "== shutting the VM down"
virsh -c qemu:///system shutdown "$VM" >/dev/null 2>&1 || true
echo "OK: VM run done"
