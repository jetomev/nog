# Test Matrix — nog v1.5.3

**Date:** 2026-09-30 · **Release:** v1.5.3 — pacman's warnings one per line ([#29](https://github.com/jetomev/nog/issues/29), F-1 of v1.5.2).
**Binary under test:** `target/release/nog` at `1.5.3` for §1–§2; the installed AUR binary for §3.

## §1 · Baseline sanity

| # | Check | Expected | Result | Notes |
|---|---|---|---|---|
| 1.1 | `cargo test --release --locked` | green | **PASS** | 193 passed, 0 failed, 4 ignored (191 at v1.5.2) |
| 1.2 | Warning delta vs v1.5.2 | unchanged | **PASS** | 6 → 6 |
| 1.3 | `grep -rn "TODO\|FIXME\|XXX" src/` | empty | **PASS** | |
| 1.4 | `strings target/release/nog \| grep -i CARGO_MANIFEST_DIR` | empty | **PASS** | |
| 1.5 | `man --warnings -l nog.1` | no warnings | **PASS** | |
| 1.6 | Version sync | 1.5.3 | **PASS** | `nog --version` → `nog 1.5.3` |

## §2 · F-1 — line endings

| # | Check | Expected | Result | Notes |
|---|---|---|---|---|
| 2.1 | Bare LF to a terminal | CR LF | **PASS** | `every_line_ending_returns_to_the_left_edge_on_a_terminal` |
| 2.2 | Existing CR LF, and one split across two reads | never doubled | **PASS** | same test |
| 2.3 | A progress redraw (CR alone) | untouched | **PASS** | same test |
| 2.4 | Real child process: terminal gets CR LF, file gets bytes untouched | as stated | **PASS** | `the_relay_translates_for_a_terminal_and_not_for_a_file` |
| 2.5 | Failing direction: translation disabled | 2.1 and 2.4 fail | **PASS** | 2 of 10 handoff tests failed, then restored |

## §3 · Installed binary

| # | Check | Expected | Result | Notes |
|---|---|---|---|---|
| 3.1 | `makepkg` smoke build from the AUR recipe | builds, tests pass in `check()` | **DEFERRED** | |
| 3.2 | Installed through `nog install <file>` | `nog --version` → 1.5.3 | **DEFERRED** | |
| 3.3 | Next real `nog update` under sudo's `use_pty` | one warning per line, left-aligned | **DEFERRED** | the pipewire family releases 1 Oct, so the next run has held rows to warn about |

## Findings

None yet.
