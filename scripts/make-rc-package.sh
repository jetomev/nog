#!/usr/bin/env bash
# Build a release-candidate package of nog from the CURRENT FILES (committed
# or not), for testing before a release (v1.6.0: Javier's run with nogForge).
#
# The recipe is the AUR one (~/Programs/aur-nog/PKGBUILD), changed in two
# places only: the source is a tarball of these files instead of the signed
# release asset (which doesn't exist yet), and the version is the rc's
# (1.6.0rc1 sorts before 1.6.0, so the real release upgrades over it). build(),
# check() (every test) and package() run exactly as on the AUR. The AUR folder
# itself is never changed.
#
#   scripts/make-rc-package.sh [rc-number]      (default 1)
#
# The package lands in dist-rc/. Logs to logs/make-rc-package-<time>.log.
set -euo pipefail
cd "$(dirname "$0")/.."
mkdir -p logs
log="logs/make-rc-package-$(date +%Y%m%d-%H%M%S).log"
ln -sfn "$(basename "$log")" logs/make-rc-package-latest.log
exec > >(tee "$log") 2>&1

rc="${1:-1}"
NOG="$PWD"
OUT="$NOG/dist-rc"
WORK="$(mktemp -d)"
trap 'rm -rf --one-file-system "$WORK"' EXIT
mkdir -p "$OUT"
base="$(sed -n 's/^version = "\([0-9.]*\).*/\1/p' Cargo.toml | head -1)"
ver="${base}rc$rc"
echo "nog $ver from the files as they are now ($(git log --format='%h' -1) + $(git status --porcelain | wc -l) changed files)"

python3 - ~/Programs/aur-nog/PKGBUILD "$WORK/PKGBUILD" "$ver" <<'PY'
import re, sys
src, out, ver = sys.argv[1:]
s = open(src).read()
s = re.sub(r"^pkgver=.*$", f"pkgver={ver}", s, flags=re.M)
s = re.sub(r"^pkgrel=.*$", "pkgrel=1", s, flags=re.M)
s = re.sub(r"^source=\(.*?\)$", f'source=("nog-{ver}.tar.gz")', s, flags=re.M | re.S)
s = re.sub(r"^sha256sums=\(.*?\)$", "sha256sums=('SKIP')   # local test build: not signed", s, flags=re.M | re.S)
s = re.sub(r"^validpgpkeys=\(.*?\)$", "", s, flags=re.M | re.S)
open(out, "w").write(s)
PY
tar czf "$WORK/nog-$ver.tar.gz" --transform "s,^\.,nog-$ver," --exclude=./target --exclude=./.git \
    --exclude=./dist-rc --exclude=./logs .
(cd "$WORK" && makepkg -f --noconfirm 2>&1 | grep -vE '^\s+(Compiling|Downloaded|Downloading|Checking|Fresh)' )
cp "$WORK"/nog-"$ver"-*.pkg.tar.zst "$OUT/" 2>/dev/null
rm -f "$OUT"/nog-debug-*
ls -la "$OUT"/nog-"$ver"-*.pkg.tar.zst
echo "OK: built nog $ver into dist-rc/"
