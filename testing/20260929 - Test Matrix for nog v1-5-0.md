# Test Matrix — nog v1.5.0

**Date:** 2026-09-29 · **Release:** v1.5.0 ([#17](https://github.com/jetomev/nog/issues/17)), carrying the tags
v1.4.2 ([#16](https://github.com/jetomev/nog/issues/16), [#14](https://github.com/jetomev/nog/issues/14)) and
v1.4.3 ([#19](https://github.com/jetomev/nog/issues/19)–[#22](https://github.com/jetomev/nog/issues/22)).
**Binary under test:** `target/release/nog` at `1.5.0` for §1–§6; the installed AUR binary for §7.

Run against [RELEASE-CHECKLIST.md](RELEASE-CHECKLIST.md). Results are recorded as they fell.
A check that needs the installed binary and a root step is **DEFERRED** until the dogfood,
not passed in advance.

---

## §1 · Baseline sanity

| # | Check | Expected | Result | Notes |
|---|---|---|---|---|
| 1.1 | `cargo test --release --locked` | green | **PASS** | 172 passed, 0 failed, 3 ignored (135 at v1.4.1 → 152 → 166 → 172) |
| 1.2 | Warning delta vs v1.4.1 | unchanged | **PASS** | 6 → 6, from cargo's own summary line. One new unused import appeared mid-cycle and was removed |
| 1.3 | `grep -rn "TODO\|FIXME\|XXX" src/` | empty | **PASS** | |
| 1.4 | `strings target/release/nog \| grep -i CARGO_MANIFEST_DIR` | empty | **PASS** | |
| 1.5 | `man --warnings -l nog.1` | no warnings | **FAIL → fixed** | One `cannot adjust line` warning. See **M-1** |
| 1.6 | Version sync: Cargo.toml, Cargo.lock, nog.conf, nog.1, README badge, README config sample | 1.5.0 | **PASS** | README's captured `nog update` transcript keeps the version it was captured under, per the checklist |

## §2 · #16 — undeclared library links

| # | Check | Expected | Result | Notes |
|---|---|---|---|---|
| 2.1 | ELF reader on built 64-bit and 32-bit fixtures | `DT_NEEDED` read correctly | **PASS** | unit tests; static, object, non-ELF and truncated files skipped without panicking |
| 2.2 | The August 29 break, declarations only | no hold (the bug) | **PASS** | `declarations_alone_miss_the_ffmpeg_obs_break` — the failing direction, so 2.3 means something |
| 2.3 | The August 29 break, with the scan | `libbluray` held, coupled to `ffmpeg-obs` | **PASS** | `the_binary_scan_catches_the_ffmpeg_obs_break` |
| 2.4 | Live scan vs `readelf` on this machine | same linkers | **PASS** | `libbluray.so.4` → `ffmpeg-obs`, `ffmpeg4.4` both ways; `libGL.so.1` (32-bit) → 2 packages both ways. libbluray's own tools excluded by design (it ships the library) |
| 2.5 | Scan cost | small, only when triggered | **PASS** | 143,000 → 46,000 files opened after skipping data extensions; 0.27s warm. Cold estimated ~7s, **not measured** (needs a cache drop, which needs root) |
| 2.6 | Real update plan with no library drop | no scan | **PASS** | 54 pending, none drop a soname; no "Checking installed programs" line; plan in 4.7s |
| 2.7 | Scan fires during a real update | the notice line appears | **CANNOT TEST** | Needs a pending update that drops a library version. None on this machine today |

## §3 · #14 — prompt line

| # | Check | Expected | Result | Notes |
|---|---|---|---|---|
| 3.1 | Continue prompt ends its line on non-interactive stdin | newline before the next message | **CANNOT TEST** | Reached only after a source step fails, which cannot be triggered on demand. Checked by reading the code |

## §4 · #19–#22 — the run log

| # | Check | Expected | Result | Notes |
|---|---|---|---|---|
| 4.1 | Held rows on a run that installed | `held` | **PASS** | `held_rows_say_held_on_a_run_that_installed` |
| 4.2 | Three sources, three step results | installed / did not complete + detail / not run | **PASS** | `each_source_gets_its_own_steps_result` |
| 4.3 | Skipped unknown, cancelled run | `skipped`, `cancelled`, `held` | **PASS** | |
| 4.4 | Two `snapd` rows told apart | `pacman` and `snap` | **PASS** | unit test; the report tables now read the same field |
| 4.5 | Failure reason picked from real process output | the `error:` line | **PASS** | real `sh` child; pacman's trailing "Errors occurred" summary does not win; makepkg's `==> ERROR:` found |
| 4.6 | A question with no newline reaches the user at once | < 0.5s | **PASS** | Checked in the failing direction: a sabotaged relay took 1.0s and the test failed |
| 4.7 | Upgrade day on a real log file | old rows untouched, blank line, new header | **PASS** | `20260929 nog-update.csv` on this machine |
| 4.8 | Real rows under the new header | `pacman`/`aur` sources, `held` outcomes | **PASS** | 53 `pacman,held`, 1 `aur,held` |
| 4.9 | Reboot advice levels | `IMPORTANT` / `NOTE` carried to their lines | **PASS** | unit test |
| 4.10 | Reboot log written during a real run | a row in `nog-reboot.csv` | **CANNOT TEST** | Needs a Tier 1 reboot-class package to be installed |

## §5 · #17 — install a package file

| # | Check | Expected | Result | Notes |
|---|---|---|---|---|
| 5.1 | Names and files mixed | refused, exit 1 | **PASS** | live: `nog install gimp ./nog-1.4.1-1-x86_64.pkg.tar.zst` |
| 5.2 | Mistyped path | "no such package file", exit 1 | **PASS** | live |
| 5.3 | Real file: identity and tier from the file | `'nog' 1.4.1-1 (local file) is Tier 3` | **PASS** | live; handed to `sudo pacman -U`, stopped at the password (no terminal) |
| 5.4 | Field labels parsed with a colon inside a value | name and version only | **PASS** | unit test |

## §6 · Documentation

| # | Check | Expected | Result | Notes |
|---|---|---|---|---|
| 6.1 | README changelog: two most recent only | v1.5.0, v1.4.3 | **PASS** | v1.4.2 moved to docs/CHANGELOG.md |
| 6.2 | Roadmap: v2 arc renumbered | C3 → v1.6.0 | **PASS** | |
| 6.3 | Privilege sections match the code | `update` always uses `sudo pacman` | **FAIL → fixed** | See **M-2** |
| 6.4 | Design doc versions | current or marked | **FAIL → fixed** | See **M-3** |

## §7 · Dogfood — the installed AUR binary (root steps run by the maintainer)

| # | Check | Expected | Result | Notes |
|---|---|---|---|---|
| 7.1 | `nog --version` after a fresh helper install | `nog 1.5.0` | **DEFERRED** | |
| 7.2 | `nog install <file>` on the `makepkg` smoke build | installs through `pacman -U` | **DEFERRED** | the first release installed by its own feature |
| 7.3 | pacman's own `Proceed? [Y/n]` visible during `nog update` | shown before the answer is typed | **DEFERRED** | the relay sits in its path; needs a run with Ready packages |
| 7.4 | Run log rows from a real install | `installed`, `held`, sources correct | **DEFERRED** | |

---

## Findings

| ID | Check | Severity | Description | Status |
|---|---|---|---|---|
| M-1 | 1.5 | low | The man page listed the twelve run-log column names as one unbreakable word; groff warned `cannot adjust line`. Introduced in the v1.4.3 docs. | FIXED in v1.5.0 |
| M-2 | 6.3 | medium | README and man page said nog runs `sudo pacman` for `update` "only when no AUR helper is configured". Since v1.3.0 the update step always hands the official repositories to `sudo pacman -Syu` itself. A false statement about when nog uses root, predating this release. | FIXED |
| M-3 | 6.4 | low | `docs/v2-design.md` cycle table carried version numbers locked on 2026-08-10, two shifts out of date before today. Marked as historical with a pointer to the live roadmap; the locked content is untouched. | FIXED |

**Process note.** M-1 was caught by the checklist's formatting gate on the next release, not on the
tag that introduced it — the v1.4.2 and v1.4.3 tags ran the tests but not the man-page check.
M-2 was found by reading the privilege section against the code while adding the new `pacman -U`
line, not against the diff.
