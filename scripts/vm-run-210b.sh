#!/usr/bin/env bash
# nog 2.1.0 rc2 in the KognogOS VM (kognog-hypeforge): the three things rc1's run could not prove.
#
#   S0  F-25 (#61): the plan, read-only — perl, apparmor, subversion as nog 2.1.0 sees them
#   S1  CONTROL, nog 2.0.0 (the VM's own): a real `nog update` at a keyboard (vm-drive answers the
#       prompts as they appear), sudo set to forget the password after 1 minute, snapd built from
#       the AUR. Expected: a password prompt AFTER snapd's build — the 2026-10-22 failure (F-21)
#   S2  the same with rc2's binary: NO password prompt after the build (--sudoloop); then the
#       computer-wide update record /var/lib/nog/last-update exists (pkg-140)
#
# The VM starts from snapshot nog-210-before for S1 and again for S2 (S1 really updates it), and
# goes back to it at the end (which also undoes the throwaway password). proton-ge-custom-bin is kept back (a huge download, no build).
# The leftover 1.9.0 rule /etc/sudoers.d/90-test-nopasswd is moved aside, so sudo really asks.
#
#   scripts/vm-run-210b.sh
#
# Logs to logs/vm-run-210b-<time>.log (+ -latest) and logs/vm-210b-<case>.log.
set -uo pipefail
cd "$(dirname "$0")/.."
mkdir -p logs
log="logs/vm-run-210b-$(date +%Y%m%d-%H%M%S).log"
ln -sfn "$(basename "$log")" logs/vm-run-210b-latest.log
exec > >(tee "$log") 2>&1
VM=kognog-hypeforge
# A throwaway password for the VM's user, made fresh for this run and set inside the VM (which goes
# back to its snapshot at the end). The one the older scripts carried never matched the VM: their
# benches ran with a no-password rule, so sudo never asked (found 2026-10-23).
VMPW="$(openssl rand -hex 8)"
V() { virsh -c qemu:///system "$@"; }
X() { scripts/vm-exec.py --timeout "${T:-900}" -- "$@"; }
J() { scripts/vm-exec.py --timeout "${T:-900}" -- "runuser -u javier -- env HOME=/home/javier bash -lc '$1'"; }
# D <case> '<command>' [REGEX ANSWER ...]: at a keyboard, sudo's prompt answered with the VM password
D() {
  local id="$1" cmd="$2"; shift 2
  local rules=""
  for r in "$@"; do rules="$rules '$r'"; done
  echo; echo "################ $id: $cmd"
  scripts/vm-exec.py --timeout 3600 -- "runuser -u javier -- env HOME=/home/javier vm-drive --timeout 3400 'cd ~ && $cmd' '\\[sudo\\] password for' '$VMPW' $rules" > "logs/vm-210b-$id.log"
  sed 's/\x1b\[[0-9;?]*[a-zA-Z]//g; s/\r//g' "logs/vm-210b-$id.log" | grep -E '^(nog:|==> (Done|Stopped|Finished making|Making package))|password for|---- exit' | tail -n 25
  # Where the password prompts fell, against snapd's build
  python3 -I - "logs/vm-210b-$id.log" <<'EOF'
import re, sys
t = open(sys.argv[1], errors="replace").read()
t = re.sub(r"\x1b\[[0-9;?]*[a-zA-Z]", "", t)
prompts = [m.start() for m in re.finditer(r"\[sudo\] password for", t)]
aur = [m.start() for m in re.finditer(r"nog: Handing off \d+ AUR", t)]
making = [m.start() for m in re.finditer(r"==> Making package", t)]
built = [m.end() for m in re.finditer(r"==> Finished making: snapd", t)]
a = aur[0] if aur else len(t); mk = making[0] if making else len(t)
print(f"  password prompts: {len(prompts)} · before the AUR step: {sum(p < a for p in prompts)}"
      f" · between nog's hand-off and the first build: {sum(a <= p < mk for p in prompts)}"
      f" · during the builds: {sum(mk <= p < (built[-1] if built else len(t)) for p in prompts)}"
      f" · after snapd's build: {sum(bool(built) and p > built[-1] for p in prompts)}")
print(f"  snapd built: {'yes' if built else 'NO'}")
EOF
}
fresh_vm() {
  V destroy "$VM" >/dev/null 2>&1 || true
  V snapshot-revert "$VM" nog-210-before || exit 1
  V start "$VM" >/dev/null || exit 1
  for i in $(seq 1 60); do
    V qemu-agent-command "$VM" '{"execute":"guest-ping"}' >/dev/null 2>&1 && break
    sleep 5
  done
  sleep 10
  scripts/vm-put.py target/release/nog /tmp/nog.new >/dev/null
  scripts/vm-put.py --mode 755 scripts/vm-drive.py /usr/local/bin/vm-drive >/dev/null
  X 'pacman -Sy >/dev/null 2>&1; mv /etc/sudoers.d/90-test-nopasswd /root/90-test-nopasswd.aside 2>/dev/null
     printf "Defaults:javier timestamp_timeout=1\n" > /etc/sudoers.d/91-short-timestamp && chmod 440 /etc/sudoers.d/91-short-timestamp && visudo -cf /etc/sudoers.d/91-short-timestamp
     rm -f /var/lib/nog/last-update /home/javier/.local/share/nog/last-update; echo ready'
  X "echo 'javier:$VMPW' | chpasswd && echo 'a throwaway password for this run'"
  # yay's own menus answered in its settings (a driver typing at them lost the keystroke on the
  # first run: the control hung 56 minutes at "Packages to cleanBuild?")
  J 'yay --save --cleanmenu=false --diffmenu=false --editmenu=false --removemake >/dev/null 2>&1 || yay --save --answerclean None --answerdiff None --answeredit None --removemake >/dev/null 2>&1; grep -E "\"(cleanmenu|diffmenu|editmenu|answerclean|answerdiff|removemake)\"" ~/.config/yay/config.json | tr -d " \n"; echo'

  scripts/vm-exec.py --input "$VMPW
" -- 'runuser -u javier -- sudo -S -k -v 2>/dev/null && echo "sudo takes it" || echo "sudo REFUSES it"; runuser -u javier -- sudo -K; true'
}
RULES=('Begin the handoff' 'y' 'Proceed with installation' 'y' '\[N\]one \[A\]ll \[Ab\]ort' '' 'Remove make dependencies' 'n')

echo "== S0 · F-25: the plan as rc2 reads it (nothing changes)"
fresh_vm
X 'nog --version; pacman -Q perl apparmor subversion snapd 2>&1'
J '/tmp/nog.new update --json 2>/dev/null' | python3 -I -c '
import json,sys
raw=sys.stdin.read(); d=json.loads(raw[raw.index("{"):raw.rindex("}")+1])
for b in ("ready","held"):
    for r in d.get(b,[]):
        if r.get("name") in ("perl","apparmor","subversion","snapd","glibc"): print(" ",b,r.get("name"),r.get("old"),"->",r.get("new"),"·",r.get("note"))
print("  ready",len(d.get("ready",[])),"· held",len(d.get("held",[])))'

if [ "${ONLY_S3:-}" != yes ]; then
echo; echo "== S1 · CONTROL: nog 2.0.0, sudo forgets after 1 minute"
X 'nog --version'
D S1 'nog update --keep proton-ge-custom-bin' "${RULES[@]}"
X 'pacman -Q snapd; ls -l /var/lib/nog/last-update 2>&1 | tail -1'

echo; echo "== S2 · rc2: the same, with --sudoloop"
fresh_vm
X 'mv -f /tmp/nog.new /usr/bin/nog && chmod 755 /usr/bin/nog && nog --version'
D S2 'nog update --keep proton-ge-custom-bin' "${RULES[@]}"
X 'pacman -Q snapd perl; echo "the record for the whole computer:"; ls -l /var/lib/nog/last-update 2>&1; echo "the per-user one (should be absent):"; ls -l /home/javier/.local/share/nog/last-update 2>&1'
J 'f=$(ls -t ~/.local/share/nog/logs/*nog-runs.csv | head -1); tail -n 2 "$f"'
fi

echo; echo "== S3 · rc2, sudo at its normal memory (Arch: 5 minutes): what yay was started with, and its keep-awake calls"
fresh_vm
X 'rm -f /etc/sudoers.d/91-short-timestamp; mv -f /tmp/nog.new /usr/bin/nog && chmod 755 /usr/bin/nog && nog --version
   nohup bash -c "for i in \$(seq 1 900); do ps -eo pid,ppid,etimes,args | grep -E \"[y]ay |sudo .*-v\" | grep -v grep >> /root/watch.txt; sleep 1; done" >/dev/null 2>&1 &
   echo "watcher started"'
D S3 'nog update --keep proton-ge-custom-bin' "${RULES[@]}"
X 'echo "yay as nog started it:"; grep -oE "yay .*" /root/watch.txt | sort -u | head -5
   echo "keep-awake calls (sudo -v) seen, by process id:"; grep -E "sudo .*-v" /root/watch.txt | awk "{print \$1, \$4, \$5, \$6, \$7}" | sort -u | head -10'

echo; echo "== VM: back to nog-210-before"
V destroy "$VM" >/dev/null 2>&1 || true
V snapshot-revert "$VM" nog-210-before && echo "restored"
echo "Done. Log: $log"
