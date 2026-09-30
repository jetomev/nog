# Test Matrix — nog v1.5.2

**Date:** 2026-09-30 · **Release:** v1.5.2 — a hold always ends ([#27](https://github.com/jetomev/nog/issues/27), `priority-1`).
**Binary under test:** `target/release/nog` at `1.5.2` for §1–§3; the installed AUR binary for §4.

Run against [RELEASE-CHECKLIST.md](RELEASE-CHECKLIST.md). Results are recorded as they fell.
A check that needs the installed binary and a root step is **DEFERRED** until the dogfood,
not passed in advance.

---

## §1 · Baseline sanity

| # | Check | Expected | Result | Notes |
|---|---|---|---|---|
| 1.1 | `cargo test --release --locked` | green | **PASS** | 191 passed, 0 failed, 4 ignored (172 at v1.5.1) |
| 1.2 | Warning delta vs v1.5.1 | unchanged | **PASS** | 6 → 6, from cargo's own summary line |
| 1.3 | `grep -rn "TODO\|FIXME\|XXX" src/` | empty | **PASS** | |
| 1.4 | `strings target/release/nog \| grep -i CARGO_MANIFEST_DIR` | empty | **PASS** | |
| 1.5 | `man --warnings -l nog.1` | no warnings | **PASS** | |
| 1.6 | Version sync: Cargo.toml, Cargo.lock, nog.conf, nog.1, README badge, README config sample | 1.5.2 | **PASS** | `nog --version` → `nog 1.5.2` |

## §2 · #27 — the rule, in unit tests

| # | Check | Expected | Result | Notes |
|---|---|---|---|---|
| 2.1 | The treadmill under the OLD rule (no record): a new Tier 1 build every 5 days for 60 days | held every time — the bug | **PASS** | `the_treadmill_never_released_under_the_old_rule`: the failing direction, so 2.2 means something |
| 2.2 | Same package, clocked from the first sighting (day 0), newest build 9 days old, day 61 | Ready | **PASS** | `counted_from_the_first_sighting_it_is_released` — the worked example in #27 |
| 2.3 | Window over, newest build 2 days old, Tier 1 | held, safety wait 5 days | **PASS** | `a_build_from_yesterday_waits_out_the_safety_wait` |
| 2.4 | Window still open | countdown from the first sighting | **PASS** | `the_window_still_runs_from_the_first_sighting` |
| 2.5 | A record newer than the build date | never stricter than the old rule | **PASS** | `a_later_sighting_never_makes_a_hold_longer_than_before` |
| 2.6 | Candidate version mismatch with a record | Unknown, as before | **PASS** | `the_version_guard_still_wins` |
| 2.7 | Record: a new build keeps the first sighting; an upgrade resets it; an installed package leaves it | as stated | **PASS** | three `sightings` tests |
| 2.8 | A source that did not answer this run keeps its clocks | AUR record survives | **PASS** | `an_unchecked_source_keeps_its_clocks` (the #25 interaction) |
| 2.9 | Run logs read by their own header, both layouts, quoted notes | parsed | **PASS** | `both_log_layouts_are_read_by_their_own_header`, `…respects_source…` |
| 2.10 | Record file round-trips; a damaged line is skipped | as stated | **PASS** | |
| 2.11 | Table notes lead with the countdown | as stated | **PASS** | `a_long_wait_says_so_and_still_leads_with_its_countdown` |

## §3 · #27 — on this desktop's real data (report only, nothing installed)

`./target/release/nog update < /dev/null` at 15:22 — stops at "Begin the handoff?" with no input.

| # | Check | Expected | Result | Notes |
|---|---|---|---|---|
| 3.1 | linux-zen + headers (7.0.5 → 7.2.7, Tier 1) | Ready | **PASS** | `waiting since Jul 29 · 8 newer versions skipped`. Under v1.5.1 the same run said `20 days remaining` |
| 3.2 | mesa + lib32-mesa (Tier 1) | Ready together | **PASS** | `waiting since Jul 29 · 5 newer versions skipped` |
| 3.3 | linux-lts + headers: window over, newest build 6 days old | held, safety wait | **PASS** | `1 day · newest build too new · waiting since Jul 29 · 11 newer versions skipped` |
| 3.4 | mkinitcpio: window over, newest build young | held, safety wait | **PASS** | `3 days · newest build too new · waiting since Aug 13 · 2 newer versions skipped` |
| 3.5 | systemd family: still inside its window since the first sighting | held, countdown from Sep 13 | **PASS** | `12 days remaining · waiting since Sep 13 · 1 newer version skipped` |
| 3.6 | A package with one new version and no history | unchanged wording | **PASS** | e.g. glibc `26 days remaining`, pipewire `1 day remaining` |
| 3.7 | Hold record written | `~/.local/state/nog/holds.tsv`, one line per pending update | **PASS** | 69 lines + 2 header lines for 69 pending |
| 3.8 | Run log note | the same wording | **PASS** | `…,ready,linux-zen,…,waiting since Jul 29 · 8 newer versions skipped,cancelled,` |
| 3.9 | Nothing installed | `Cancelled — nothing was installed.` | **PASS** | |

## §4 · Installed binary (dogfood)

| # | Check | Expected | Result | Notes |
|---|---|---|---|---|
| 4.1 | `makepkg` smoke build from the AUR recipe | builds, tests pass in `check()` | **PASS** | source fetched from the release, sha256 and signature verified, 191 passed in `check()` |
| 4.2 | Installed through `nog install <file>` | `nog --version` → 1.5.2 | **PASS** | 30 Sep ~15:35, by Javier; pacman 1.5.1-1 → 1.5.2-1. `/etc/nog/nog.conf` was unmodified, so pacman replaced it: the safety-wait keys are in it now |
| 4.3 | First real `nog update` with the fix: the kernels and mesa install | installed, reboot advised | **DEFERRED** | Javier's run; a 7.0 → 7.2 kernel jump with a DKMS rebuild |
| 4.4 | After reboot: running kernel 7.2.7, NVIDIA module loaded | verified | **DEFERRED** | |
| 4.5 | Fresh install from the AUR through yay | `nog --version` → 1.5.2 | **DEFERRED** | |

## Findings

None yet.
