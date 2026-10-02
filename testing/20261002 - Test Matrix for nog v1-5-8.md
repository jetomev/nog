# Test Matrix — nog v1.5.8 (F-5 #37, F-6 #38, F-7 #39, F-8 #40, the run frame)

**Why:** what Javier's own run of v1.5.7 raised ([v1.5.7 matrix](20261002%20-%20Test%20Matrix%20for%20nog%20v1-5-7.md) §3), and his rulings the same day: no hold list on a one-package install, installs that ask, messages with a designed form, and the same start and end on every run.
**Where:** the KognogOS VM `kognog-hypeforge`, reset to `clean-install-3` with `scripts/vm-prep-nog.sh` (nog 1.5.7 from the release, then the local build copied over it).
**Who:** §2 Claude, through the guest agent with recorded screens (`logs/vm-158-*.log`). §3 Javier, at the VM's keyboard.

## §1 · Baseline sanity

| # | Check | Expected | Result | Notes |
|---|---|---|---|---|
| 1.1 | `cargo test --release --locked` | green | **PASS** | 224 passed, 0 failed, 5 ignored (221 at v1.5.7) |
| 1.2 | Warning delta vs v1.5.7 | unchanged | **PASS** | 6 → 6 |
| 1.3 | `grep -rn "TODO\|FIXME\|XXX" src/` | empty | **PASS** | |
| 1.4 | `strings target/release/nog \| grep -i CARGO_MANIFEST_DIR` | empty | **PASS** | |
| 1.5 | `man --warnings -l nog.1` | no warnings | **PASS** | |
| 1.6 | Version sync | 1.5.8 | **PASS** | |

## §2 · Claude, in the VM

| # | Check | Expected | Result | Notes |
|---|---|---|---|---|
| 2.1 | `nog install cowsay` on a fresh install | no banner of `nog update`, no SUMMARY / ON HOLD tables, no `ignoring package upgrade` lines; a notice naming what installs | **PASS** | F-5; pacman.log: `-Sy --needed --ignore <held> <ready>` |
| 2.2 | pacman's question for the update step | below its package table | **PASS** | F-8; above it in the recording before the fix |
| 2.3 | The install itself | pacman shows its table and asks | **PASS** | F-6; v1.5.7 ran `-S --noconfirm` |
| 2.4 | Answer **n** to the install question | "Not installed: cmatrix … Nothing was changed", nothing installed | **PASS** | F-6, F-7 |
| 2.5 | `nog remove sl` | pacman asks | **PASS** | F-6; was `-Rs --noconfirm` |
| 2.6 | Every notice | one blank line before, one after, `==>` heading | **PASS** | F-7; unit tests lock the rule |
| 2.7 | search, install (y / n), remove, update | each starts with the banner and ends with "Done" or "Stopped", the logs, the thank-you; one line each in `nog-runs.csv` | **PASS** | the run frame |
| 2.8 | List refresh before an install | no progress bars, so the next notice keeps its blank line at the bottom of a full terminal | **PASS** | `-Sy --noprogressbar` |

## §3 · Javier, at the VM

| # | Check | Expected | Result | Notes |
|---|---|---|---|---|
| 3.1 | `nog install cowsay`, yes throughout | banner, notices, READY TO INSTALL table, holds as one line, the install asks, the ending | **PASS** | 12:17; *"i tried it all and looks very good"* |
| 3.2 | His own further tries | as designed | **PASS** | *"tried it all"* |
| 3.3 | `nog update` | the full report, the frame | **PASS** | 12:32, `update,0,done`; nothing was Ready, so no pacman step. *"all looks much better, always room for improvement, but one step-at-a-time"* |

## Known, not in this release

- The AUR helper, Flatpak and Snap steps still relay stderr through nog, so a helper's question could race its output the way F-8 did (noted in #40).
- The "update completed" record is per user (`~/.local/share/nog/last-update`): a run as root does not see javier's record. That only ever causes an extra safe update, never a skipped one.
