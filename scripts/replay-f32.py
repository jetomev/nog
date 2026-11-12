#!/usr/bin/env python3
"""Replay of the F-32 rule (#69) over a real `nog update` run, read only.

Reads the run's Ready and Held tables from its log, each Ready package's OLD
version from pacman's cache (its library files and declared provides, as they
were before the run), the sync database's build dates, and which held packages'
programs on disk link those libraries. Prints what the rule would have held.
Run after the fact, when the Ready packages are already installed.

  python3 -I scripts/replay-f32.py "<log>" [<checkupdates sync dir>]
"""
import os, re, subprocess, sys, tarfile

log, syncdir = sys.argv[1], (sys.argv[2] if len(sys.argv) > 2 else f"/tmp/checkup-db-{os.getuid()}/sync")
text = re.sub(r"\x1b\[[0-9;]*m", "", open(log, errors="replace").read()).replace("\r", "").splitlines()

def table(start):
    i = next(n for n, l in enumerate(text) if l.startswith(start))
    rows, n = [], i + 5  # title, rule, blank, header, dashes
    while n < len(text) and text[n].strip() and not text[n].startswith("-"):
        p = text[n].split()
        if len(p) >= 4: rows.append((p[0], p[2], p[3]))
        n += 1
    return rows
ready, held = table("READY TO INSTALL"), table("ON HOLD FROM INSTALL")
held_names = {h[0] for h in held}

built = {}
for db in os.listdir(syncdir):
    if db.endswith(".db"):
        with tarfile.open(f"{syncdir}/{db}") as t:
            for m in t.getmembers():
                if m.name.endswith("/desc"):
                    d = t.extractfile(m).read().decode()
                    name = re.search(r"%NAME%\n(.+)", d).group(1)
                    b = re.search(r"%BUILDDATE%\n(\d+)", d)
                    if b: built[name] = int(b.group(1))

upstream = lambda v: v.rsplit("-", 1)[0]
cache = "/var/cache/pacman/pkg"
libs_of = {}
for name, old, new in ready:
    if upstream(old) == upstream(new): continue
    f = next((f"{cache}/{x}" for x in os.listdir(cache)
              if x.startswith(f"{name}-{old}-") and x.endswith(".pkg.tar.zst")), None)
    if not f:
        print(f"  (no cached {name}-{old}: not checked)"); continue
    names = subprocess.run(["bsdtar", "-tf", f], capture_output=True, text=True).stdout.split()
    info = subprocess.run(["bsdtar", "-xOf", f, ".PKGINFO"], capture_output=True, text=True).stdout
    declared = {re.split(r"[=<>]", l.split(" = ", 1)[1])[0] for l in info.splitlines() if l.startswith("provides = ")}
    libs = set()
    for p in names:
        m = re.fullmatch(r"usr/lib(32)?/([^/]+\.so)\.([\d.]+)", p)
        if m and m.group(2) not in declared: libs.add(f"{m.group(2)}.{m.group(3)}")
    if libs: libs_of[name] = libs

wanted = {l for s in libs_of.values() for l in s}
linkers = {}  # basename -> held packages linking it
for h in held_names:
    d = next((f"/var/lib/pacman/local/{x}" for x in os.listdir("/var/lib/pacman/local") if x.rsplit("-", 2)[0] == h), None)
    if not d or not os.path.exists(f"{d}/files"): continue
    files = [l for l in open(f"{d}/files").read().split("%FILES%\n", 1)[-1].split("\n\n")[0].splitlines()
             if re.match(r"(usr/lib|usr/lib32|usr/bin|usr/libexec|opt)/", l) and not l.endswith("/")]
    elfs = []
    for f in files:
        p = "/" + f
        try:
            if os.path.isfile(p) and not os.path.islink(p) and open(p, "rb").read(4) == b"\x7fELF": elfs.append(p)
        except OSError: pass
    for i in range(0, len(elfs), 200):
        out = subprocess.run(["readelf", "-d", *elfs[i:i+200]], capture_output=True, text=True).stdout
        for n in re.findall(r"\(NEEDED\)\s+Shared library: \[([^\]]+)\]", out):
            if n in wanted: linkers.setdefault(n, set()).add(h)

print(f"Ready {len(ready)} · Held {len(held)} · Ready with undeclared libraries and an upstream change: {len(libs_of)}")
holds = 0
for name, libs in sorted(libs_of.items()):
    users = sorted({u for l in libs for u in linkers.get(l, ()) if built.get(u, 0) >= built.get(name, 1 << 62)})
    if users:
        holds += 1
        print(f"  WOULD HOLD {name:24} for {', '.join(users)}  ({', '.join(sorted(libs))})")
    else:
        print(f"  stays      {name:24} ({', '.join(sorted(libs))})")
print(f"\nwould hold {holds}")
