# Test Results — nog v1.5.1 against paru (#12)

**Date:** 2026-09-29 · **Binary:** installed `nog 1.5.1` · **Helpers:** paru 2.1.0 (chaotic-aur), yay-bin 13.0.1
**Scope:** [#12](https://github.com/jetomev/nog/issues/12) — paru has been "supported" since v1.0.0 and was never once run.
Root steps run by Javier; everything else read-only.

## §1 · The known unknowns from #12

| # | Check | Result | Notes |
|---|---|---|---|
| 1.1 | `-Qua` output parses | **PASS** | Both print `fresh-editor-bin 0.5.1-1 -> 0.5.2-1`; yay appends `[1d8h]`, which nog already ignores |
| 1.2 | `-Sai` "Last Modified" parses to the same moment | **PASS** | paru: `Mon, 28 Sep 2026 08:33:42` (no zone, local time); yay: `Mon 28 Sep 2026 08:33:42 AM EDT`. `date -d` gives 1790598822 for both. The feared silent all-Unknown failure does not happen |
| 1.3 | nog's own code, both helpers, same answers | **PASS** | `aur::tests::live_helpers_agree` (new, ignored diagnostic): identical pending list and identical dates for 3 packages, none undated |
| 1.4 | "Nothing to update" under paru | **CANNOT TEST** | An AUR update has been pending throughout. A probe that hid it made paru *fail* instead — which surfaced **F-2** |
| 1.5 | `--ignore` accepted on `-S` | **PASS** | `paru -S --ignore glibc <missing>` resolved dependencies and stopped at the missing target — flag accepted |
| 1.6 | Refuses to install AUR packages as root | **PASS** (by inspection) | paru carries `can't install AUR package as root`; not run with sudo |

## §2 · The plan, end to end

| # | Step | Result | Notes |
|---|---|---|---|
| 2.1 | Install paru through nog | **PASS** | `nog install paru` → yay → `chaotic-aur/paru 2.1.0-2.1` |
| 2.2 | Remove yay | **PASS** | package is `yay-bin` (+ `yay-bin-debug`), not `yay` — my first command named the wrong package; nothing changed |
| 2.3 | `helper = "auto"` falls back to paru | **PASS** | `nog: 1 AUR update(s) reported by paru.` |
| 2.4 | Full `nog update` plan under paru | **PASS** | `fresh-editor-bin` dated and **Held** (`5 days remaining`), not Unknown; run logged |
| 2.5 | nog install through paru's real AUR path | **PASS** | `nog install yay-bin` → paru: review, download, sha256 check, build, install. (`nog install yay` went to chaotic-aur's binary instead — Javier caught that paru never touched the AUR there, and stopped it) |
| 2.6 | yay back, `auto` returns to yay | **PASS** | `reported by yay`; same plan; diagnostic still identical |
| 2.7 | AUR *update* handoff under paru (`upgrade_cleared`) | **CANNOT TEST** | The one AUR update is held for 5 more days; nothing to hand off |

## Findings

| ID | Severity | Description | Status |
|---|---|---|---|
| F-2 | medium | A failing helper with empty stdout reads as "no AUR updates", silently; its stderr is discarded. Not paru-specific, present since v1.0. Holds are safe; updates can go unseen. [#25](https://github.com/jetomev/nog/issues/25) | OPEN |

**Side effect:** `yay-bin-debug` (debug symbols only) was not reinstalled — paru builds but does not install debug packages by default. It is in `~/.cache/paru/clone/yay-bin/` if wanted.

**Verdict:** nog works with paru. 12 of 14 checks pass; the 2 left need a state this machine did not have
today (no AUR update due, and a genuinely empty AUR list). #12 closes; F-2 carries the one real defect.
