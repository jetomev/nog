# nog — v1.0.5 dogfood test results

Companion to [`20260707 - Test Matrix for nog v1-0-5.md`](20260707 - Test Matrix for nog v1-0-5.md).

## Run metadata

- **Package under test:** `nog 1.0.5-1` delivered from AUR (`yay -S nog` after `sudo pacman -Rns nog nog-debug`)
- **AUR source:** `aur.archlinux.org/nog.git` commit `c01c362 nog 1.0.5 — hold windows dated from stale sync DBs`
- **GitHub source:** `jetomev/nog` tag `v1.0.5` → commit `8e8c8a3 release: v1.0.5 prep — PKGBUILD bump + test matrix`
- **Tarball sha256:** `2442bb5dbb3d71b6cbbb8e27f28377fd7d2354d771cb55adab41011c9bf0504f` (verified by makepkg from the GitHub tag tarball)
- **Install method:** `yay -S nog` against the freshly pushed AUR package, reproducing the public install path
- **Test run:** 2026-07-08 (same-day with the v1.0.4 → v1.0.5 hotfix push)
- **Tester:** Javier (`jetomev`) + Claude (Anthropic), per [[nog-development-discipline]] and [[github-surface-completeness]]
- **Trigger:** v1.0.5 fixes the stale-DB hold bug — hold windows dated from the *predecessor's* build date because build dates were read from `/var/lib/pacman/sync` (root-synced only, during the post-report handoff) while candidates came from `checkupdates`' fresh private DB. This dogfood validates the fix on the binary users will actually install, on the same host that surfaced the bug.

## The live before/after — the reason this release exists

Two `nog update` runs on the same host, hours apart, straddling the upgrade:

**Before — `nog 1.0.4-1` (AUR), 2026-07-08 ~10:11 AM.** 12 packages reported **Ready**, *every one wrong*. All 12 were built 0–3 days earlier and belonged in Held; v1.0.4 dated each from its stale predecessor:

| Package | v1.0.4 verdict | True build date | Correct verdict |
|---|---|---|---|
| archlinux-keyring 20260707.1-1 | Ready · "19 days past window" | 2026-07-08 00:36 (≈8h old) | Held |
| pinentry 1.3.3-1 | Ready · "317 days past window" | 2026-07-07 10:58 | Held |
| openssh 10.4p1-2 | Ready · "90 days past window" | 2026-07-07 07:48 | Held |
| python-cffi 2.1.0-1 | Ready · "200 days past window" | 2026-07-07 02:33 | Held |
| zxing-cpp 3.1.0-1 | Ready · "134 days past window" | 2026-07-07 14:07 | Held |
| *(+7 more, all 0–3 days old)* | Ready · absurd figures | 2026-07-05…08 | Held |

No Tier 1 escaped that run — but only by luck (their predecessors happened to be recent, so the wrong clock looked plausible). That is the self-masking pattern the changelog post-mortem describes.

**After — `nog 1.0.5-1` (AUR), 2026-07-08 ~1:23 PM.** The 12 wrongly-Ready packages had been installed in the interim; four new Tier 3 packages appeared since. Result:

```
Held (19):
  ... (all kernels, systemd family, mesa, plasma-desktop held as before)
  libxfont2 2.0.7-1 -> 2.0.8-1            [Tier 3 · 6 days remaining]
  xorg-server 21.1.23-1 -> 21.1.24-1     [Tier 3 · 6 days remaining]
  xorg-server-common 21.1.23-1 -> ...    [Tier 3 · 6 days remaining]
  xorg-xwayland 24.1.12-1 -> 24.1.13-1   [Tier 3 · 6 days remaining]
nog: nothing to install right now — all pending updates are held.
```

**Ready: 0. Absurd "past window" figures: none.** Every first-sighting Tier 3 package now enters Held with a sane countdown instead of being waved through.

## Independent recomputation (fresh checkupdates DB, same run)

Read directly from `${TMPDIR:-/tmp}/checkup-db-<uid>/sync/*.db` — the exact snapshot that produced the candidate list — and recomputed by hand:

| Package | Tier / window | Build date (fresh DB) | Elapsed → remaining | nog showed | Match |
|---|---|---|---|---|---|
| xorg-server 21.1.24-1 | 3 / 7d | 2026-07-08 02:03 | 1d → 6d | 6 days remaining | ✓ |
| libxfont2 2.0.8-1 | 3 / 7d | 2026-07-08 02:10 | 1d → 6d | 6 days remaining | ✓ |
| xorg-xwayland 24.1.13-1 | 3 / 7d | 2026-07-08 02:07 | 1d → 6d | 6 days remaining | ✓ |
| linux-zen 7.1.3.zen1-1 | 1 / 30d | 2026-07-07 01:04 | 2d → 28d | 28 days remaining | ✓ |

Note `linux-zen` read **25 days remaining** under v1.0.4 that morning and **28** under v1.0.5 — the countdown correctly re-measured from the candidate's true builddate (the "days remaining may shift a few days" behavior documented in the changelog).

## Section status

| Section | Title | Status | Notes |
|---|---|---|---|
| 1 | Baseline sanity | pass | `nog 1.0.5` confirmed; man page header reads `.TH NOG 1 "July 2026" "nog v1.0.5"`; `/etc/nog` clean |
| 2–14 | Existing surfaces | not run (this dogfood) | No v1.0.5 code-path changes to search / install / remove / pin / privilege model / config / expert mode / AUR integration. Covered by prior dogfoods |
| 15 | Binary + filesystem invariants | pass | `/usr/bin/nog` unchanged; slim-deps contract preserved (no new runtime deps); **29/29** unit tests in the AUR build's `check()` phase |
| 16–17 | Kernel/headers + pkgbase coupling | pass (regression) | All coupling verdicts unchanged from v1.0.4 (kernels, systemd, mesa, lib32-* families held at correct tiers) |
| **18a** | Fresh-snapshot hold evaluation | **pass — core of the v1.0.5 fix** | Held countdowns computed from the checkupdates snapshot; four new Tier 3 packages correctly Held at 6 days; independent recomputation matches on all sampled packages |
| **18b** | Fallback path | not run (live) | Fallback + warning path is unit-tested; not exercised live (snapshot present as expected) |
| **18c** | Candidate-version guard | pass (unit) | 4 guard unit tests green in `check()`; no live version-mismatch case present this run |
| **18d** | Legacy debug surfaces | not run (this dogfood) | `_debug-dates`/`_debug-hold` retain the system-DB path by design (matrix 18.10) |

## Verdict

**PASS — released.** The published AUR binary reproduces the fix: 0 wrongly-Ready packages, every first-sighting update correctly Held, all countdowns matching an independent recomputation from the fresh DBs. The exact bug that produced "317 days past window" for a day-old build in the morning run is gone by the afternoon run on the installed package.
