# Test Matrix — nog v1.5.6 (#30, #31)

**Why here:** #30 was found on a fresh KognogOS install (the hypeForge edition, kognog `d30a082`). A fix is believed only where the bug lives.
**When:** first ruled "wait for the hypeForge session, no release before"; then Javier asked for the AUR release so he can install it in the VM himself (30 Sep 2026, evening). §1 ran before release; §2–§3 are his VM run.
**Binary:** §1 `target/release/nog` at 1.5.6; §2–§3 nog 1.5.6 from the AUR, installed in the VM.

## §1 · Baseline sanity

| # | Check | Expected | Result | Notes |
|---|---|---|---|---|
| 1.1 | `cargo test --release --locked` | green | **PASS** | 219 passed, 0 failed, 5 ignored (217 at v1.5.5) |
| 1.2 | Warning delta vs v1.5.5 | unchanged | **PASS** | 6 → 6 |
| 1.3 | `grep -rn "TODO\|FIXME\|XXX" src/` | empty | **PASS** | |
| 1.4 | `strings target/release/nog \| grep -i CARGO_MANIFEST_DIR` | empty | **PASS** | |
| 1.5 | `man --warnings -l nog.1` | no warnings | **PASS** | |
| 1.6 | Version sync | 1.5.6 | **PASS** | |
| 1.8 | Release + AUR | signed assets; AUR smoke build green; ready-built package attached for the VM | **PASS** | GitHub v1.5.6 Latest, source sha256 verified after download; AUR `0aed844`, 219 passed in `check()`; `nog-1.5.6-1-x86_64.pkg.tar.zst` + `.sig` attached, downloaded copy identical |
| 1.7 | On this desktop (lists 5 h old, no keyring pending) | neither new path triggers; `nog update` report unchanged | **PASS** | report: 60 held, 0 ready, no key-store message |

## Starting point
Best: revert `kognog-hypeforge` to the snapshot `clean-install-2` (a fresh install: the disc's own package lists, never refreshed). Ask the hypeForge session first; that revert discards whatever the VM holds now.

## §2 · #30 — package lists before an install

| # | Check | Expected | Result | Notes |
|---|---|---|---|---|
| 2.1 | Reproduce with the installed (old) nog: `nog install cowsay` | fails: files not found on the mirrors | **PASS** | nog 1.5.5 fails: `target not found: cowsay`. Build 3 ships **no** package lists at all (`/var/lib/pacman/sync` missing), so the failure is "not found" rather than "file gone from the mirror" — same bug, harder form |
| 2.2 | New nog (1.5.6 from the AUR): `nog install cowsay` | "the package lists are N days old — refreshing them, the safe way", a tier-aware update, then cowsay installs | **PASS** | "the package lists are missing — refreshing them, the safe way"; update: 3 Ready installed (networkmanager, libnm, ppp), 25 held; "Package lists are current — now installing cowsay"; cowsay 3.8.4-1 installed. Needed F-1 + F-2 workarounds first. nog 1.5.6 came from the release's ready-built package, not the AUR |
| 2.3 | Same, with everything held | lists refreshed with `pacman -Sy`, then the install | **DEFERRED** | did not occur: 3 updates were Ready |
| 2.4 | Answer **n** at "Begin the handoff?" | install stops: "the update did not complete … not installing cowsay" | **PASS** | "Cancelled — nothing was installed" then the expected message; exit 1. See F-3 for a cancel at pacman's own prompt, and F-4 for an early failure |
| 2.5 | Run again right away | lists fresh: installs at once, no update | **PASS** | `nog install sl`: straight to pacman, no update |

## §3 · #31 — keys first

| # | Check | Expected | Result | Notes |
|---|---|---|---|---|
| 3.1 | A pending `archlinux-keyring` | Ready, `keys · never held, installed first`; its own pacman step before the main one | **PASS** | set up by installing 20260902-1 from the Arch archive; Ready with the note, "Installing the new keys first (archlinux-keyring)", own pacman step, then the main step. **Limit:** 20260909 was 23 days old, past its window anyway — the "never held" rule itself rests on the unit tests |
| 3.2 | Key store missing (`/etc/pacman.d/gnupg` moved aside) | nog explains, runs `pacman-key --init` + `--populate`, carries on | **PASS** | plain-words message, init + populate (188 keys), "key store ready", then the update check. Original restored (189 keys) |

## Claude's run · 2026-10-02 · VM `kognog-hypeforge` from `clean-install-3`

Driven through the qemu guest agent with `scripts/vm-exec.py`, as root. Answers go through `script` with pauses: typed all at once, nog's own prompt takes every line and pacman's "Proceed?" gets nothing (a harness limit, not a nog bug). Prompts were visible every time — the desktop's missing-prompt bug did not reproduce here either. Logs: `logs/vm-*.log`.

**Result: 6 PASS, 1 DEFERRED (2.3), 4 findings.**

| # | Finding | Severity |
|---|---|---|
| F-1 (#33) | nog's signing key cannot be fetched by pacman: keyserver.ubuntu.com has no copy (404); keys.openpgp.org serves it **without a user ID** (email never verified), which gpg skips. A downloaded nog package cannot be installed on a fresh KognogOS. Worked around by copying the key in from the desktop | high |
| F-2 (#34, KognogOS#9) | A fresh KognogOS has no `fakeroot`. `checkupdates` needs it, so `nog update` — and the #30 path — stop at "checkupdates failed: Cannot find the fakeroot binary". The desktop never saw this because it has base-devel. Worked around with `pacman -Sy fakeroot`, then the lists deleted again | high |
| F-3 (#35) | Declining pacman's own "Proceed?" leaves the lists **fresh** but the Ready updates **not installed**. A `nog install` within the next day then skips the update — the partial upgrade #30 exists to prevent. Seen: cowsay installed without the update | medium |
| F-4 (#36) | When the update fails early (e.g. F-2), nog exits from inside the update, so "not installing cowsay" never prints | low |
