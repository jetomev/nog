# Test Matrix — nog v1.5.5

**Date:** 2026-09-30 · **Release:** v1.5.5 — `nog clean`, tier-aware cleanup of pacman's download cache ([#15](https://github.com/jetomev/nog/issues/15)).
**Binary under test:** `target/release/nog` at `1.5.5` for §1–§3; the installed AUR binary for §4.

## §1 · Baseline sanity

| # | Check | Expected | Result | Notes |
|---|---|---|---|---|
| 1.1 | `cargo test --release --locked` | green | **PASS** | 217 passed, 0 failed, 5 ignored (204 at v1.5.4; the new ignored test is the live `vercmp` comparison, run separately in 2.2) |
| 1.2 | Warning delta vs v1.5.4 | unchanged | **PASS** | 6 → 6 |
| 1.3 | `grep -rn "TODO\|FIXME\|XXX" src/` | empty | **PASS** | |
| 1.4 | `strings target/release/nog \| grep -i CARGO_MANIFEST_DIR` | empty | **PASS** | |
| 1.5 | `man --warnings -l nog.1` | no warnings | **PASS** | |
| 1.6 | Version sync | 1.5.5 | **PASS** | |

## §2 · Version order

| # | Check | Expected | Result | Notes |
|---|---|---|---|---|
| 2.1 | Hard cases, both directions | same as `/usr/bin/vercmp` | **PASS** | 15 pairs incl. `1.0` > `1.0a`, `1.0_rc1` > `1.0`, epochs, `+r24+g…` git versions, leading zeros |
| 2.2 | Every version pair in this machine's real cache | same as `/usr/bin/vercmp` | **PASS** | `vercmp_agrees_with_pacman_over_the_real_cache` (ignored by default; run with `--ignored`) |
| 2.3 | File names read from the right | name, `[epoch:]pkgver-pkgrel` | **PASS** | hyphenated names, epochs, `any` arch; `.sig`, `.part`, `download-…` rejected |

## §3 · Retention

| # | Check | Expected | Result | Notes |
|---|---|---|---|---|
| 3.1 | Tier 1: installed + 2 older kept | as stated | **PASS** | unit |
| 3.2 | Tier 3: installed only; `.sig` goes with its package | as stated | **PASS** | unit |
| 3.3 | Held package: installed kept, newer download kept | as stated | **PASS** | unit, linux-lts shape |
| 3.4 | Uninstalled package: every file removed | as stated | **PASS** | unit |
| 3.5 | Keep 0 in config | treated as 1: installed never removed | **PASS** | unit |
| 3.6 | Stale download folders: a day old, exact name shape | as stated | **PASS** | a path-traversal name rejected |
| 3.7 | Real cache report (answered no) | sane, nothing removed | **PASS** | 18.4 GB: 10.8 GB removable (1,845 versions + 123 folders), 7.6 GB kept; removable + kept = total. 0.3 s |
| 3.8 | "No longer installed" vs `paccache -d -u -k0` | same | **PASS** | both 346 versions, 1.9 GB (paccache: 1.88 GiB) |
| 3.9 | Spot checks | as the rules say | **PASS** | linux-zen 3 cached, none removed (Tier 1); pipewire held at 1.6.8: 1.6.7, 1.6.8, 1.6.9 all kept (Tier 2 + newer download); 7zip at 26.03: 26.01 and 26.02 removed (Tier 3) |

## §4 · Installed binary

| # | Check | Expected | Result | Notes |
|---|---|---|---|---|
| 4.1 | `makepkg` smoke build from the AUR recipe | builds, tests pass in `check()` | **PASS** | release fetched, sha256 + signature verified, 217 passed in `check()`; pushed to the AUR |
| 4.2 | Installed through `nog install <file>` | `nog --version` → 1.5.5 | **PASS** | 30 Sep ~18:35, by Javier; `/etc/nog/nog.conf` carries the new `[clean]` section |
| 4.3 | `nog clean`, answered yes, one password | files removed, cache smaller by about the reported size | **PASS** | Javier, ~18:40 ("wow! awesome!"). `du`: 19G → 7.7G; 6,840 entries → 3,041. `paccache -d -u -k0` afterwards: no candidates |
| 4.4 | Afterwards: every installed version still in the cache | `pacman -Q` versions all present | **PASS** | 1,424 of 1,439 installed versions were cached before, 1,424 after, identical list. linux-zen keeps 3; pipewire (held) keeps 1.6.7 / 1.6.8 / 1.6.9; 7zip keeps only 26.03. 10 `download-…` folders remain, all from today — the one-day rule, as designed |

## Findings

None. Observation, not a nog finding: pacman left a new `download-…` folder in the cache on most runs today (10 between 15:33 and 18:34). The next `nog clean` a day later removes them.
