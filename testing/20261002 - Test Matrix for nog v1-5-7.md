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

## §3 · Javier's run (VM, nog 1.5.7 from the GitHub release)

**Setup (Claude, through the guest agent, 2026-10-02):** VM reverted to `clean-install-3`; key added from `github.com/jetomev.gpg` (`pacman-key --add` + `--lsign-key`); release package + `.sig` downloaded; `pacman -Sy` + `pacman -U` (the package was accepted with the GitHub key); then the package lists and any `last-update` record deleted, so the checks start from a fresh install's state. No AUR helper or build tools in the VM, and Chaotic-AUR does not carry nog, so "from the AUR" became the ready-built release package.

| # | Check | Expected | Result | Notes |
|---|---|---|---|---|
| 3.1 | `pacman -Q fakeroot` after installing nog 1.5.7 | installed, pulled in by nog | **PASS** | absent before; `core/fakeroot` came in with nog (Claude, setup) |
| 3.2 | `nog install cowsay`: **y** at "Begin the handoff?", **n** at pacman's "Proceed?" | "not installing cowsay" | **PASS** | Javier, 10:59, second attempt (VM reset + same setup): he stopped at **nog's** gate instead — run log: libnm, networkmanager, ppp `cancelled`; pacman never ran (pacman.log quiet after setup); lists still missing; no record written; cowsay not installed. The stop at *pacman's* prompt rests on Claude's §2.2 — ruled enough by Javier, not repeated a third time |
| 3.3 | `nog install cowsay` again, answering yes throughout | "no record of a completed update — updating first", then cowsay installs | **PASS** | Javier, 10:49–10:51, as the first run (he answered y at both prompts): lists missing → update (libnm, networkmanager, ppp installed, 25 held) → record written 10:51:26 → cowsay. Raised **F-5**: the full update report is too much for a one-package install |
| 3.4 | `nog install cowsay` answering yes throughout, then `nog install sl` right after | cowsay after the update; then sl at once, no update | **PASS** | Javier, 11:02–11:03: `pacman -Syu` (libnm, networkmanager, ppp) → cowsay 11:02:47, record written 11:02:47 → sl 11:03:08 with no update in between (pacman.log) |
| 3.5 | `sudo pacman -Sy`, then `nog install cmatrix` | "the package lists changed after the last completed update — updating first" | **DEFERRED** | Javier |

## Findings from Javier's run

| # | Finding | Severity |
|---|---|---|
| F-5 | `nog install <one package>` that has to update first prints the whole `nog update` report: SUMMARY, READY, ON HOLD (25 rows), UNKNOWN. Javier: *"If I am asking to install one package, I should not receive a message with all available and hold packages, that doesn't make a lot of sense."* Ruled: show only what will install, plus one line for the holds | low (presentation) |
