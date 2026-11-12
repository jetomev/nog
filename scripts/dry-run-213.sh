#!/usr/bin/env bash
# Dry run of nog 2.1.3's undeclared-library rule (F-32 #69) on THIS desktop, read only: the installed nog
# and the freshly built one (target/release/nog) each print `nog update --json`, which stops before
# any question, from the same copy of the hold record in a throwaway HOME. Nothing is installed.
#
#   R1  the OpenSSL 4 rebuilds (curl, git, krb5, libssh2, openssh, sudo, qca-qt6) stay Held, coupled
#       to openssl (2.1.1's fix, kept)
#   R2  dotnet-runtime and dotnet-host: Ready with the installed 2.1.1 (pacman then pulled openssl-3 in
#       and refused, Javier's run 2026-11-11 20:02), Held with the new one
#   R2b simdjson: when an update of it is pending (after Javier's repair put 1:5.0.5 back and
#       `nog lock` is lifted), Held, coupled to nodejs; otherwise said, not guessed
#   R3  what else moved, listed by name; nothing may move from Held to Ready
#   R4  pacman's view, worked out here without nog: for every package the new nog leaves Ready, the
#       packages pacman would install NEW for it; none may carry a library a held package still owns
#
#   scripts/dry-run-213.sh                      (the build vs the installed nog)
#   NOG_NEW=/usr/bin/nog NOG_OLD=<2.1.2> scripts/dry-run-213.sh   (after installing 2.1.3)
#
# Logs to logs/dry-run-213-<time>.log (+ -latest link); the last line says OK.
set -uo pipefail
cd "$(dirname "$0")/.."
mkdir -p logs
log="logs/dry-run-213-$(date +%Y%m%d-%H%M%S).log"
ln -sfn "$(basename "$log")" logs/dry-run-213-latest.log
exec > >(tee "$log") 2>&1

NEW="${NOG_NEW:-$PWD/target/release/nog}"
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

DB="$(ls -d /tmp/checkup-db-"$(id -u)"/sync 2>/dev/null)"
python3 -I - "$HOME/old.json" "$HOME/new.json" "$DB" <<'PY'
import json, sys, os, tarfile, re
old, new = (json.load(open(f)) for f in sys.argv[1:3])
syncdir = sys.argv[3]
def where(d, n):
    for b in ("ready", "held", "unknown"):
        for e in d[b]:
            if e["name"] == n: return b, e.get("coupled_to")
    return None, None
fail = 0
print("\n################ R1: the seven stay held, coupled to openssl")
for n in ["curl", "git", "krb5", "libssh2", "openssh", "sudo", "qca-qt6"]:
    o, n2 = where(old, n), where(new, n)
    ok = n2[0] == "held" and n2[1] == "openssl"
    fail |= not ok
    print(f"  {n:16} {str(o[0]):6} -> {n2[0]} (coupled to {n2[1]})  {'ok' if ok else 'FAIL'}")
print("\n################ R2: the dotnet pair")
for n in ["dotnet-runtime", "dotnet-host"]:
    o, n2 = where(old, n), where(new, n)
    ok = n2[0] == "held"
    fail |= not ok
    print(f"  {n:16} {str(o[0]):6} -> {n2[0]} (coupled to {n2[1]})  {'ok' if ok else 'FAIL'}")
print("\n################ R2b: simdjson (F-32)")
o, n2 = where(old, "simdjson"), where(new, "simdjson")
if n2[0] is None:
    print("  no simdjson update pending on this desktop now: not testable live (the unit tests replay it)")
else:
    ok = n2[0] == "held" and n2[1] == "nodejs"
    fail |= not ok
    print(f"  simdjson         {str(o[0]):6} -> {n2[0]} (coupled to {n2[1]})  {'ok' if ok else 'FAIL'}")
print("\n################ R3: everything that moved from Ready to Held")
was_ready = {e["name"] for e in old["ready"]}
moved = sorted((e["name"], e.get("coupled_to"), e["days_remaining"]) for e in new["held"] if e["name"] in was_ready)
for m in moved: print(f"  {m[0]:28} coupled to {m[1]} ({m[2]} days)")
print(f"\n  ready {len(old['ready'])} -> {len(new['ready'])} · held {len(old['held'])} -> {len(new['held'])} · moved {len(moved)}")
back = sorted(e["name"] for e in new["ready"] if e["name"] not in was_ready)
if back: print("  FAIL: Ready now but not before:", back); fail = 1

print("\n################ R4: what pacman would install new for the Ready set (read from the databases)")
def fields(text):
    out, key = {}, None
    for line in text.splitlines():
        if line.startswith("%") and line.endswith("%"): key = line.strip("%"); out[key] = []
        elif line and key: out[key].append(line)
    return out
local = {}
for d in os.listdir("/var/lib/pacman/local"):
    p = f"/var/lib/pacman/local/{d}/desc"
    if os.path.exists(p):
        f = fields(open(p).read()); local[f["NAME"][0]] = f.get("PROVIDES", [])
sync = {}
for db in sorted(os.listdir(syncdir)):
    if not db.endswith(".db"): continue
    with tarfile.open(f"{syncdir}/{db}") as t:
        for m in t.getmembers():
            if m.name.endswith("/desc"):
                f = fields(t.extractfile(m).read().decode())
                sync.setdefault(f["NAME"][0], (f.get("PROVIDES", []), f.get("DEPENDS", [])))
name = lambda e: re.split(r"[<>=]", e)[0]
inst_prov = {p for l in local.values() for p in l}
inst_prov_names = {name(p) for p in inst_prov}
def satisfied(e):
    exact = "=" in e and "<" not in e and ">" not in e
    if exact: return e in inst_prov or (name(e) in local and not any(name(p) == name(e) for p in sync.get(name(e), ([], []))[0]))
    return name(e) in local or name(e) in inst_prov_names
held_installed = {e["name"] for e in new["held"]}
owned = {}  # soname a held package still owns after this run -> that package
for h in held_installed:
    for p in local.get(h, []):
        if ".so=" in p: owned[p] = h
pulled = {}
for e in new["ready"]:
    r = e["name"]
    queue = [d for d in sync.get(r, ([], []))[1] if not satisfied(d)]
    seen = set()
    while queue:
        d = queue.pop()
        n = name(d)
        cands = [n] if n in sync and n not in local else [k for k, (pv, _) in sync.items() if k not in local and any(name(x) == n for x in pv)]
        for c in cands:
            if c in seen: continue
            seen.add(c); pulled.setdefault(c, set()).add(r)
            queue += [x for x in sync[c][1] if not satisfied(x)]
if not pulled: print("  nothing new")
for c, by in sorted(pulled.items()):
    clash = sorted({owned[p] for p in sync[c][0] if p in owned})
    bad = bool(clash)
    fail |= bad
    print(f"  {c:28} for {', '.join(sorted(by))}" + (f"  FAIL: takes files of held {clash}" if bad else ""))
print("\nOK" if not fail else "\nFAILED")
sys.exit(fail)
PY
