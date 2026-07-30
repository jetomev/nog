# nog — v1.0.8 dogfood test results

Focused run-logging release: every `nog update` now appends a CSV record — one row per pending package mirroring the report tables, plus the run's outcome — to a per-day file (`YYYYMMDD nog-update.csv`) with 90-day retention. No behavior change to the hold/tier engine; the regression surface is the new pure `runlog` module (7 new unit tests), which runs in the AUR build's `check()` on every install.

## Run metadata

- **Package under test:** `nog 1.0.8-1`, built + installed via `makepkg -fsi` from the AUR clone.
- **GitHub source:** `jetomev/nog` tag `v1.0.8` → commit `b8286fc fix(runlog): .csv extension — spreadsheet apps refuse to import .log`
- **Tarball sha256:** `014b4d962a735fabe94e1377312aaffe9bf2f3dcc4c8c75399cdcbb86465dcde` (verified by makepkg from the GitHub tag tarball)
- **`check()` phase:** `cargo test --release --locked` → **42 passed, 0 failed** during the AUR build (was 35; +7 for the `runlog` module)
- **Test run:** 2026-07-29, on the KognogOS desktop (Ryzen 7 7700, 31 GiB)
- **Tester:** Javier (`jetomev`) + Claude (Anthropic), per [[nog-development-discipline]] and [[github-surface-completeness]]
- **Trigger:** the roadmap's planned v1.0.8 item — CSV run logging with 3-month retention, foreshadowed in the v1.0.7 changelog.

## The live evidence

One day-file — `~/.local/share/nog/logs/20260729 nog-update.csv` — accumulated **four runs** during the release day, covering three of the five outcome types and proving same-day appending across binaries (dev build → AUR binary):

| Run | Time | Binary | Rows | Outcome |
|---|---|---|---|---|
| 1 | 08:32 PM | dev build (Phase 2 smoke) | 76 | `cancelled` (EOF at the Proceed gate) |
| 2 | 08:52 PM | dev build (README capture) | 76 | `cancelled` |
| 3 | ~09:16 PM | **AUR binary 1.0.8-1** | 76 | `installed` (3 Ready went through) |
| 4 | 09:17 PM | **AUR binary 1.0.8-1** | 73 | `all held` (76 − 3 installed) |

Outcome tally across the file (header excluded): 152 `cancelled` + 76 `installed` + 73 `all held` = 301 rows, one header line, field count 10 on every line.

Verified in detail:

- **Header written once** at file creation; runs 2–4 appended without duplicating it.
- **Full run context on every line** (`date,time,user`) — the file stays self-describing under `cat`/`grep`.
- **Faithful table mirror** — spot-checked rows including the v1.0.6 coupling note (`coupled to libnm · 5 days`), which also exercises the CSV interpunct path (no quoting needed, field count intact).
- **Outcome arithmetic** — run 3 (`installed`, 76 rows) followed by run 4 (`all held`, 73 rows): the pending set shrank by exactly the 3 Ready packages the transaction upgraded. The log answers "did I actually install that day?" — the design goal.
- **Closing line** — `nog: run logged to /home/jetomev/.local/share/nog/logs/20260729 nog-update.csv` printed in subtext after `Update finished!`.
- **Spreadsheet import** — Javier confirmed the file opens cleanly in desktop apps ("CSV looking great in terminal and common apps"). This was the point of finding F-1 below.
- **Retention** — no files older than the 90-day cutoff existed, so no prune fired (prune-candidate selection is unit-tested; the live path logged nothing, as expected).

## Findings

**F-1 (fixed pre-ship): `.log` extension blocked spreadsheet import.** The roadmap draft named the file `YYYYMMDD nog-update.log`; Javier's first dogfood pass found that common apps refuse to import a `.log` even though the content is CSV. Renamed to `.csv` in `b8286fc` before the AUR release — code, retention matcher, tests, README, man page, `nog.conf` comment. The `v1.0.8` tag was moved to include the fix (pre-release, nothing had shipped).

**F-2 (process, no code change): makepkg reinstalled the stale pre-fix package.** After the tag moved, a second `makepkg -si` in the AUR clone short-circuited on the previously built `nog-1.0.8-1` package ("already built") and reinstalled the `.log`-writing binary. Recovery: remove the cached `nog-1.0.8*.tar.gz` source and `nog-*.pkg.tar.zst` packages, then `makepkg -fsi` — the new PKGBUILD sha256 makes the rebuild self-verifying. Lesson for future same-version tag moves (which should stay rare): always `-f` and clear the cached tarball.

One orphan artifact from F-2 — the `20260729 nog-update.log` written by the pre-fix binary — deleted after verification (wrong suffix, so retention would never have collected it).

## Verdict

**No code findings against the shipped binary. v1.0.8 cleared for AUR release.**
