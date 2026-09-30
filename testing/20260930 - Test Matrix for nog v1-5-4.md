# Test Matrix — nog v1.5.4

**Date:** 2026-09-30 · **Release:** v1.5.4 — summary table + Source column ([#28](https://github.com/jetomev/nog/issues/28)), `could not check` ([#25](https://github.com/jetomev/nog/issues/25)), no-keyboard AUR install refused ([#26](https://github.com/jetomev/nog/issues/26)).
**Binary under test:** `target/release/nog` at `1.5.4` for §1–§4; the installed AUR binary for §5.

## §1 · Baseline sanity

| # | Check | Expected | Result | Notes |
|---|---|---|---|---|
| 1.1 | `cargo test --release --locked` | green | **PASS** | 204 passed, 0 failed, 4 ignored (193 at v1.5.3) |
| 1.2 | Warning delta vs v1.5.3 | unchanged | **PASS** | 6 → 6. A 7th (`repo_order` unused) existed mid-build and went when the summary used it |
| 1.3 | `grep -rn "TODO\|FIXME\|XXX" src/` | empty | **PASS** | |
| 1.4 | `strings target/release/nog \| grep -i CARGO_MANIFEST_DIR` | empty | **PASS** | |
| 1.5 | `man --warnings -l nog.1` | no warnings | **PASS** | |
| 1.6 | Version sync | 1.5.4 | **PASS** | `nog --version` → `nog 1.5.4` |

## §2 · #28 — summary table and Source column

| # | Check | Expected | Result | Notes |
|---|---|---|---|---|
| 2.1 | Summary rows in pacman.conf order, then AUR, Flatpak, Snap; an absent source has no row | as stated | **PASS** | `the_summary_counts_by_repository_in_pacman_conf_order` |
| 2.2 | Numbers right-aligned, totals correct, both rules the same width | as stated | **PASS** | `the_summary_aligns_and_totals` |
| 2.3 | "Ask you" column only when something is Unknown | as stated | **PASS** | `the_ask_you_column_appears_only_when_needed` |
| 2.4 | Every source has a word; AUR never blank; Arch `snapd` vs the snap `snapd` | `extra` vs `Snap` | **PASS** | `every_source_has_a_word_and_aur_is_never_blank` |
| 2.5 | Colour never moves a column | stripped colour output = plain output | **PASS** | `colour_never_moves_a_column` |
| 2.6 | Real data, this desktop | 7 rows, 60 held, repositories correct | **PASS** | core 13, extra 38, multilib 6, chaotic-aur 2, AUR 1, Flatpak 0, Snap 0. `lib32-glibc` reads `core` — checked with `pacman -Si`: Arch does keep it in core |

## §3 · #25 — a failing helper

| # | Check | Expected | Result | Notes |
|---|---|---|---|---|
| 3.1 | yay's real empty answer (exit 1, no stdout, no stderr) | nothing to update | **PASS** | unit test from the 30 Sep probe |
| 3.2 | paru's real failure (exit 1, no stdout, `error: failed to run: pacman --query …`) | failure | **PASS** | unit test from the 30 Sep probe |
| 3.3 | yay `->` notices and `warning:` lines | not a failure | **PASS** | |
| 3.4 | Live: a fake `yay` on PATH fails `-Qua` | warning quotes the error; summary `AUR could not check` | **PASS** | 1.5.4 dev binary |
| 3.5 | Failing direction: the same fake against installed 1.5.3 | the bug: `0 AUR update(s)` | **PASS** | `nog: 0 AUR update(s) reported by yay.` |

## §4 · #26 — no keyboard

| # | Check | Expected | Result | Notes |
|---|---|---|---|---|
| 4.1 | Unit: only names missing from the sync DBs need the keyboard | as stated | **PASS** | `only_aur_names_need_the_keyboard` |
| 4.2 | Live: `nog install grubforge </dev/null` | refused before the helper, plain reason, exit 1 | **PASS** | With a stale binary the first try reached yay's menu (`-> EOF`) — that was the pre-fix build, since `cargo test` does not rebuild the binary. Rebuilt: refused |

## §5 · Installed binary

| # | Check | Expected | Result | Notes |
|---|---|---|---|---|
| 5.1 | `makepkg` smoke build from the AUR recipe | builds, tests pass in `check()` | **PASS** | release fetched, sha256 + signature verified, 204 passed in `check()`; pushed to the AUR as `4db32d7` |
| 5.2 | Installed through `nog install <file>` | `nog --version` → 1.5.4 | **DEFERRED** | |
| 5.3 | Next real `nog update`: summary, Source column, warnings one per line (v1.5.3) | as stated | **DEFERRED** | the pipewire family releases 1 Oct |

## Findings

- **T-1 (test tooling, fixed before release):** `tally-matrix.py` counted 2 false FAILs in this matrix — check 3.2's Expected cell says "failure" and check 3.5's title starts "Failing direction". It took the first cell *starting* with a verdict word. It now reads only the Result column, found from each table's header row. Re-tallying every matrix: no change except v1.5.0 (4 FAIL → 3; its Test Results carry a dated correction) and v1.5.3 (1 false FAIL, never published).
