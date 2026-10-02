#!/usr/bin/env bash
# Reset the KognogOS test VM to a fresh install with nog 1.5.7 from the GitHub
# release, optionally replace the binary with a local build, and remove the
# package lists so the first `nog install` starts from a fresh install's state.
#   scripts/vm-prep-nog.sh [path/to/nog-binary]
# Logs to logs/vm-prep-nog-<time>.log (+ -latest link); last line says OK.
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p logs
log="logs/vm-prep-nog-$(date +%Y%m%d-%H%M%S).log"
ln -sfn "$(basename "$log")" logs/vm-prep-nog-latest.log
exec > >(tee "$log") 2>&1
VM=kognog-hypeforge
virsh -c qemu:///system destroy "$VM" >/dev/null 2>&1 || true
virsh -c qemu:///system snapshot-revert "$VM" clean-install-3
virsh -c qemu:///system start "$VM" >/dev/null
for i in $(seq 1 60); do
  virsh -c qemu:///system qemu-agent-command "$VM" '{"execute":"guest-ping"}' >/dev/null 2>&1 && break
  sleep 5
done
scripts/vm-exec.py --timeout 600 'curl -s https://github.com/jetomev.gpg -o /tmp/jetomev.gpg
pacman-key --add /tmp/jetomev.gpg >/dev/null 2>&1
pacman-key --lsign-key 32E1D2AB9380BFD6BFE3BC1EAC2A3407CC070F9E >/dev/null 2>&1
cd /tmp
curl -sSLO https://github.com/jetomev/nog/releases/download/v1.5.7/nog-1.5.7-1-x86_64.pkg.tar.zst
curl -sSLO https://github.com/jetomev/nog/releases/download/v1.5.7/nog-1.5.7-1-x86_64.pkg.tar.zst.sig
pacman -Sy >/dev/null 2>&1
pacman -U --noconfirm /tmp/nog-1.5.7-1-x86_64.pkg.tar.zst >/dev/null 2>&1
rm -rf /var/lib/pacman/sync
pacman -Q nog fakeroot 2>/dev/null'
if [ -n "${1:-}" ]; then
  scripts/vm-put.py "$1" /tmp/nog.new
  scripts/vm-exec.py 'mv -f /tmp/nog.new /usr/bin/nog; nog --version'
fi
echo OK
