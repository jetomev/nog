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
| 2.1 | Reproduce with the installed (old) nog: `nog install cowsay` | fails: files not found on the mirrors | **DEFERRED** | the failing direction first |
| 2.2 | New nog (1.5.6 from the AUR): `nog install cowsay` | "the package lists are N days old — refreshing them, the safe way", a tier-aware update, then cowsay installs | **DEFERRED** | |
| 2.3 | Same, with everything held | lists refreshed with `pacman -Sy`, then the install | **DEFERRED** | may not occur naturally |
| 2.4 | Answer **n** at "Begin the handoff?" | install stops: "the update did not complete … not installing cowsay" | **DEFERRED** | |
| 2.5 | Run again right away | lists fresh: installs at once, no update | **DEFERRED** | |

## §3 · #31 — keys first

| # | Check | Expected | Result | Notes |
|---|---|---|---|---|
| 3.1 | A pending `archlinux-keyring` | Ready, `keys · never held, installed first`; its own pacman step before the main one | **DEFERRED** | a fresh install from an older disc usually has one pending |
| 3.2 | Key store missing (`/etc/pacman.d/gnupg` moved aside) | nog explains, runs `pacman-key --init` + `--populate`, carries on | **DEFERRED** | restore the original afterwards |
