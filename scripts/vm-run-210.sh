#!/usr/bin/env bash
# nog 2.1.0's real-change cases in the KognogOS VM (kognog-hypeforge): `nog update` with nobody
# at the keyboard (F-20, #56), as an installer, a timer or a script runs it. Every case runs as
# the VM's user `javier` with stdin from /dev/null, through the guest agent (no terminal at all),
# so no password is ever typed: that is the point.
#
#   Y1  no --yes, nobody at the keyboard   → "Not updating", the cause said, exit 1, nothing changed
#   Y2  --yes, sudo would ask a password   → "Not updating", the password said, exit 1, nothing changed
#   Y3  --yes, a sudo rule for pacman      → the update runs to the end: the handoff answered,
#                                            pacman --noconfirm, the AUR kept back with a hint, exit 0
#   Y4  --yes, a stand-in flatpak          → its Ready app's update fails on purpose and its Unknown
#                                            app is kept back: --yes stops at the failure, the run
#                                            exits 3 and its runs line says incomplete (F-22)
#
# The VM's current state is kept first as the snapshot `nog-210-before` (or, when that snapshot
# exists, the VM starts from it), and it is restored at the end: the VM comes out as it went in.
# The VM carries a rule from the 1.9.0 bench, /etc/sudoers.d/90-test-nopasswd (the test user may
# run anything without a password). The first run of this bench, 2026-10-23 00:04, kept it, so
# its Y2 never met a password: sudo said yes and the update ran, as nog should when sudo allows.
# Y2 and Y3 move that rule aside, so sudo really would ask; Y3 adds a rule for pacman only.
#
#   scripts/vm-run-210.sh
#
# Logs to logs/vm-run-210-<time>.log (+ -latest).
set -uo pipefail
cd "$(dirname "$0")/.."
mkdir -p logs
log="logs/vm-run-210-$(date +%Y%m%d-%H%M%S).log"
ln -sfn "$(basename "$log")" logs/vm-run-210-latest.log
exec > >(tee "$log") 2>&1
VM=kognog-hypeforge
V() { virsh -c qemu:///system "$@"; }
X() { scripts/vm-exec.py --timeout "${T:-900}" -- "$@"; }
J() { scripts/vm-exec.py --timeout "${T:-1800}" -- "runuser -u javier -- env HOME=/home/javier bash -lc '$1'"; }
# R <case> '<nog command>': run it as javier with nobody at the keyboard; show nog's own lines
R() {
  J "$2 </dev/null >/tmp/$1.out 2>&1; echo EXIT=\$?" | grep EXIT
  X "sed 's/\x1b\[[0-9;?]*[a-zA-Z]//g' /tmp/$1.out | grep -E '^(nog:|==> [A-Z][a-z]|    [A-Za-z(-])|UNKNOWN|Not updating' | grep -v '^==> \(Building\|Using\|Starting\|Generating\|Creating\|Initcpio\|WARNING\)' | head -n 60"
}
runs() { J 'f=$(ls -t ~/.local/share/nog/logs/*nog-runs.csv | head -1); tail -n 4 "$f"'; }

echo "== VM: from nog-210-before (kept the first time), then start it"
V destroy "$VM" >/dev/null 2>&1 || true
if V snapshot-info "$VM" nog-210-before >/dev/null 2>&1; then
  V snapshot-revert "$VM" nog-210-before || exit 1
else
  V snapshot-create-as "$VM" nog-210-before "the VM as it was before nog 2.1.0's --yes cases" || exit 1
fi
V start "$VM" >/dev/null || exit 1
for i in $(seq 1 60); do
  V qemu-agent-command "$VM" '{"execute":"guest-ping"}' >/dev/null 2>&1 && break
  sleep 5
done
sleep 10

echo "== the fresh binary"
scripts/vm-put.py target/release/nog /tmp/nog.new
X 'mv -f /tmp/nog.new /usr/bin/nog; chmod 755 /usr/bin/nog; nog --version; rm -f /etc/sudoers.d/90-nog-210-test'
pending() { T=900 J 'pacman -Qu 2>/dev/null | wc -l' | grep -E '^[0-9]+$' | head -1; }
echo "== the VM's lists brought current (root, not nog: so the cases have something pending)"
X 'pacman -Sy >/dev/null 2>&1; echo synced'
before=$(pending); echo "pending updates: $before"

echo; echo "################ Y1: nog update, nobody at the keyboard, no --yes"
R y1 'nog update'
echo "pending after Y1: $(pending) (was $before: nothing may change)"

echo; echo "################ Y2: nog update --yes, nobody at the keyboard, sudo would ask"
X 'mv /etc/sudoers.d/90-test-nopasswd /root/90-test-nopasswd.aside && echo "the 1.9.0 rule moved aside"'
J 'sudo -K; sudo -n pacman --version >/dev/null 2>&1; echo "sudo without a password, for pacman: $?"'
R y2 'nog update --yes'
echo "pending after Y2: $(pending) (was $before: nothing may change)"

echo; echo "################ Y3: nog update --yes, a sudo rule for pacman (this run only)"
X 'echo "javier ALL=(root) NOPASSWD: /usr/bin/pacman" > /etc/sudoers.d/90-nog-210-test && chmod 440 /etc/sudoers.d/90-nog-210-test && visudo -cf /etc/sudoers.d/90-nog-210-test'
J 'sudo -n pacman --version >/dev/null 2>&1; echo "sudo without a password, for pacman: $?"; sudo -n true 2>/dev/null; echo "for anything else: $?"'
T=1800 R y3 'nog update --yes'
echo "pending after Y3: $(pending) (was $before)"
echo "---- the runs log, last lines"
runs

echo; echo "################ Y4: nog update --yes, a stand-in flatpak whose update fails (and one Unknown app)"
X 'cat > /usr/local/bin/flatpak <<"EOS"
#!/bin/sh
# the 2.1.0 bench stand-in: two app updates, one dated (Ready) and one without a date (Unknown)
case "$1" in
  --version) echo "Flatpak 1.16.1" ;;
  remotes) echo flathub ;;
  remote-ls) printf "org.nogbench.Dated\t2.0\tflathub\norg.nogbench.NoDate\t2.0\tflathub\n" ;;
  remote-info) [ "$3" = org.nogbench.Dated ] && echo "        Date: 2026-09-01 10:00:00 +0000" || exit 1 ;;
  list) printf "org.nogbench.Dated\t1.0\norg.nogbench.NoDate\t1.0\n" ;;
  update) echo "error: the bench stand-in fails this update on purpose" >&2; exit 1 ;;
  *) exit 0 ;;
esac
EOS
chmod 755 /usr/local/bin/flatpak; echo "stand-in flatpak in place"'
J 'command -v flatpak'
R y4 'nog update --yes'
echo "---- the runs log, last lines"
runs
X 'rm -f /usr/local/bin/flatpak; echo "stand-in removed"'
echo "---- outcomes in the update log"
J 'f=$(ls -t ~/.local/share/nog/logs/*nog-update.csv | head -1); python3 -c "import csv,collections,sys; print(collections.Counter((r[\"bucket\"],r[\"outcome\"]) for r in csv.DictReader(open(sys.argv[1]))))" "$f"'
X 'rm -f /etc/sudoers.d/90-nog-210-test; mv /root/90-test-nopasswd.aside /etc/sudoers.d/90-test-nopasswd; echo "rules back as they were"'

echo; echo "== VM: back to nog-210-before (the state it came in)"
V destroy "$VM" >/dev/null 2>&1 || true
V snapshot-revert "$VM" nog-210-before && echo "restored"
echo "Done. Log: $log"
