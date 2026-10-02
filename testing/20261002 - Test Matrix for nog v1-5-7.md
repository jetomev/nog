# Test Matrix — nog v1.5.7 (F-2 #34, F-3 #35, F-4 #36)

**Why:** the fixes for what Claude's VM run of v1.5.6 found ([v1.5.6 matrix](20260930%20-%20Test%20Matrix%20for%20nog%20v1-5-6.md)).
**Where:** the KognogOS VM `kognog-hypeforge`, from snapshot `clean-install-3` (build 3, freshly installed), plus §1 on the desktop.
**Who:** §1 and §2 Claude, before release (§2 with `target/release/nog` copied in at `3dc89cb`). §3 is Javier's own run, from the AUR.

## §1 · Baseline sanity

| # | Check | Expected | Result | Notes |
|---|---|---|---|---|
| 1.1 | `cargo test --release --locked` | green | **PASS** | 221 passed, 0 failed, 5 ignored (219 at v1.5.6) |
| 1.2 | Warning delta vs v1.5.6 | unchanged | **PASS** | 6 → 6 |
| 1.3 | `grep -rn "TODO\|FIXME\|XXX" src/` | empty | **PASS** | |
| 1.4 | `strings target/release/nog \| grep -i CARGO_MANIFEST_DIR` | empty | **PASS** | |
| 1.5 | `man --warnings -l nog.1` | no warnings | **PASS** | |
| 1.6 | Version sync | 1.5.7 | **PASS** | Cargo.toml, Cargo.lock, nog.conf, nog.1, README badge + sample |

## §2 · The fixes in the VM (Claude, before release)

| # | Check | Expected | Result | Notes |
|---|---|---|---|---|
| 2.1 | No lists, no fakeroot: `nog install cowsay` | names fakeroot + `sudo pacman -Syu fakeroot`; "not installing cowsay" | **PASS** | F-2, F-4 |
| 2.2 | Answer y to nog, **n** to pacman's "Proceed?" | "not installing cowsay" | **PASS** | F-4 (pacman step) |
| 2.3 | `nog install cowsay` right after 2.2 | updates first ("no record of a completed update"), not a straight install | **PASS** | F-3 — v1.5.6 installed straight away here |
| 2.4 | A full run answered yes | update, record written, cowsay installs | **PASS** | |
| 2.5 | `nog install sl` right after | installs at once | **PASS** | |
| 2.6 | `pacman -Sy` outside nog, then `nog install cmatrix` | "the package lists changed after the last completed update — updating first" | **PASS** | F-3 |

## §3 · Javier's run (VM, nog 1.5.7 from the AUR)

Start: revert `kognog-hypeforge` to `clean-install-3`. F-1 (#33) is still open, so nog's key has to be added by hand first, and the AUR build needs fakeroot and base-devel anyway.

| # | Check | Expected | Result | Notes |
|---|---|---|---|---|
| 3.1 | `pacman -Q fakeroot` after installing nog 1.5.7 | installed (pulled in by nog) | **DEFERRED** | |
| 3.2 | Repeat 2.2 + 2.3 | as above | **DEFERRED** | |
| 3.3 | Repeat 2.4–2.6 | as above | **DEFERRED** | |
| 3.4 | `nog update` with a key package pending (v1.5.6 §3.1) | keys first, still | **DEFERRED** | regression check |
