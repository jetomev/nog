# Test Results — nog v1.4.1

**Date:** 2026-09-16 · **Scope:** issue #18 (snap holds not enforced) · hotfix release
**Origin:** not a scheduled matrix. A routine read of nog's own run logs found the bug;
nothing had failed visibly and nothing had been reported.

## 1 · Baseline

| # | Check | Verdict | Note |
|---|---|---|---|
| 1.1 | `cargo test --release --locked` | PASS | 135 passed, 0 failed, 2 ignored (128 → 135) |
| 1.2 | Warning delta vs v1.4.0 | PASS | 6 → 6, read from cargo's own summary line |
| 1.3 | No `TODO`/`FIXME`/`XXX` in `src/` | PASS | none |
| 1.4 | No embedded maintainer paths in the release binary | PASS | `CARGO_MANIFEST_DIR` grep empty |
| 1.5 | Man page formats without groff warnings | PASS | 0 warnings |
| 1.6 | Version sync across all surfaces | PASS | Cargo.toml, Cargo.lock, nog.conf, nog.1, README badge, README config sample |

## 2 · The bug, reproduced from the record

| # | Check | Verdict | Note |
|---|---|---|---|
| 2.1 | Run log shows a snap held | PASS | `20260915 nog-update.csv`: `held,core20,20260410,20260901,3,1 day remaining` |
| 2.2 | snapd refreshed it anyway | PASS | `snap changes` → `Auto-refresh snap "core20"`, same minute as the nog run |
| 2.3 | Installed revision moved | PASS | `snap list` → core20 at 20260901, the version nog was holding back |
| 2.4 | snapd's schedule identified | PASS | `snap refresh --time` → `timer: 00:00~24:00/4` |

## 3 · Unit coverage (selection and arithmetic)

| # | Check | Verdict | Note |
|---|---|---|---|
| 3.1 | Snaps sharing a window are grouped into one call | PASS | `hold_args_groups_snaps_sharing_a_window` |
| 3.2 | Zero-day window takes snapd's ceiling | **FAIL → FIXED** | See F-1 |
| 3.3 | Over-long window clamps to the 90-day ceiling | PASS | asking for more fails the call, leaving the snap unheld |
| 3.4 | Grouping is deterministic run-to-run | PASS | |
| 3.5 | Held non-snaps are not selected | PASS | gimp/linux-zen excluded |
| 3.6 | No held snaps → no hold call at all | PASS | the case that must never prompt for root |
| 3.7 | Name-matching ambiguity documented, not hidden | PASS | `snapd` is both an Arch package and a snap (issue #20) |

## 4 · Live verification on this machine (root steps run by the maintainer)

| # | Check | Verdict | Note |
|---|---|---|---|
| 4.1 | snapd accepts `--hold=<N>h` | PASS | `General refreshes of "hello" held until 2026-09-16T22:03:38-04:00` — exactly +1h |
| 4.2 | A hold leaves a *named* refresh unblocked | PASS | `snap refresh hello` → `snap "hello" has no updates available`, not a hold refusal |

> **4.2 is the load-bearing assumption of the whole fix.** Had a hold blocked nog's
> own named refresh, a held snap would have become permanently stuck — a worse bug
> than the one being fixed. It was taken from snapd's help text until this check;
> it is now taken from this machine.

## 5 · Not verified

| # | Item | Why |
|---|---|---|
| 5.1 | End-to-end: nog placing a hold during a real run | No snap updates are pending (`All snaps up to date`), so the path cannot fire naturally. **Deliberately not faked with stand-in commands** — driving the full flow would have written a fabricated row into the permanent run log. The selection was extracted into a pure function and tested directly instead. |
| 5.2 | Behaviour when a hold is placed and the window then expires across runs | Calendar-bound; needs a real pending snap. |

## Findings

| ID | Check | Severity | Description | Status |
|---|---|---|---|---|
| F-1 | 3.2 | high | A zero-day hold window is the Tier 1 "awaiting manual signoff" placeholder, i.e. held until a person says otherwise. Plain arithmetic (`0 × 24`, clamped to a 1-hour floor) produced a **one-hour hold** for exactly the packages carrying the most risk — a hold in name only, reproducing #18 for the worst case. Caught because the test asserted the documented promise rather than the arithmetic. | FIXED before commit |
| M-1 | docs | medium | Man page stated snap holds worked "the same way as flatpak: nog names exactly the snaps it cleared this run and nothing else" — a description of the bug written as if it were the design. | FIXED |
| M-2 | docs | medium | Man page **PRIVILEGES AND SUDO** claimed nog escalates in "exactly two places" and modifies no file other than `tier-pins.toml` — explicitly naming `/etc/pacman.conf` as untouched. nog has commented out the Chaotic-AUR section there since v1.0.9, and the README's own escalation table documented four places. A false claim about what a tool does with root. | FIXED |

**Process note.** F-1 was found by a test written to assert a promise made in a comment,
not to confirm the code's behaviour. M-1 and M-2 were found by reading the shipped docs
against the code rather than against the diff — M-2 predates this release entirely and
would not have surfaced from reviewing the change alone.
