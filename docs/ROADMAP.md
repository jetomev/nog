# nog — full roadmap history

*The README carries the upcoming work and the two most recent releases; everything older lives here, newest-first (the [locked convention](../README.md#roadmap)).*

### v1.0.8 — Released
- [x] **CSV run logging** — every `nog update` run appends to a per-day CSV log (`YYYYMMDD nog-update.csv` under `[paths] run_logs`, default `~/.local/share/nog/logs`) mirroring the update-table columns (bucket / package / old / new / tier / note) plus the banner context (date / time / user) and the run's **outcome** (`installed` / `cancelled` / `all held` / `up to date` / `handoff failed`). Retention: logs older than 90 days pruned after each write. Logging soft-fails with a warning — it never blocks an update. New pure `runlog` module with unit tests; 35 → 42.
- [x] **Dogfooded on the AUR binary (2026-07-29)** — `makepkg -fsi` ran the suite (42/42) in `check()`, installed `1.0.7-1 → 1.0.8-1`; four runs accumulated in one day-file covering three outcome types (`cancelled` ×2 dev, `installed` — 76 rows, `all held` — 73 rows after the transaction: exactly the 3 Ready packages installed), header written once, appends clean across binaries; CSV imported cleanly into desktop apps — the `.csv`-extension fix (F-1) was made pre-ship at Javier's call. [Test Results](testing/20260729 - Test Results for nog v1-0-8.md).

### v1.0.7 — Released
- [x] **Reformatted `nog update` output** — a banner header (name / Date / Time / User), **per-source counts** (official via pacman + AUR via the helper), and the Ready / Held / Unknown buckets rendered as aligned **tables** (`Package (N) | Old Version | New Version | Tier | Note`; empty sections show `(none)`). Tier is a bare per-tier-colored digit; terminal width is intentionally ignored so long version strings just widen the columns. New pre-handoff **`Proceed? [Y/n]`** review gate (yay/pacman still confirms after — two deliberate layers). New pure `format_table()` with unit tests; 33 → 35.
- [x] **Dogfooded on the AUR binary (2026-07-18)** — `makepkg -si` ran the suite (35/35) in `check()`, installed `1.0.6-1 → 1.0.7-1`; live `nog update` rendered the full report on a real 201-package set (banner, per-source counts, the three aligned tables, `(none)` Unknown, the v1.0.6 coupled row, and the `Proceed?` gate). No findings. [Test Results](testing/20260718 - Test Results for nog v1-0-7.md).

### v1.0.6 — Released
- [x] **lib32/base hold coupling ([#1](https://github.com/jetomev/nog/issues/1))** — a `lib32-<X>` and its base `<X>` are version-locked, but their hold windows are dated independently, so one could land in **Ready** while the other stayed **Held** — leaving pacman unable to satisfy the exact-version dependency and aborting the *entire* transaction (hit live on the nvidia stack). `holds::lib32_coupling_demotions()` demotes the Ready member of any split pair into Held so the pair releases together; bidirectional, and the Held row now names the package it's waiting on. 29 → 33 tests.
- [x] **Dogfooded on the AUR binary (2026-07-16)** — verified on the same host that hit the original abort, with the exact split still pending: `lib32-nvidia-utils` moved from Ready into Held as `[Tier 3 · coupled to nvidia-utils · 3 days]`, landed in the pacman ignore list, and the transaction resolved and installed 16 packages with no abort — while non-split lib32 pairs (`fontconfig`/`libffi`/`libssh2`/`p11-kit`) correctly stayed together in Ready. [Test Results](testing/20260716 - Test Results for nog v1-0-6.md).

### v1.0.5 — Released
- [x] **Phase 8 — candidate-fresh hold evaluation** — `nog update` now dates hold windows from the **same DB snapshot that produced the candidate list**: the private dbpath `checkupdates` syncs on every run (`$CHECKUPDATES_DB`, default `${TMPDIR:-/tmp}/checkup-db-<uid>/`). Previously it read `/var/lib/pacman/sync`, which only refreshes when root syncs — i.e. during the handoff *after* the report — so every first-sighting update was dated from its *predecessor's* builddate and could skip its hold entirely (975 days "past window" in the worst observed case). Falls back to the system DB with a warning if the snapshot is missing.
- [x] **Candidate-version guard** — `sync_db.rs` now reads `%VERSION%`; the new `holds::evaluate_candidate()` refuses to date a hold from a DB entry that isn't the pending candidate's exact version — mismatches route to **Unknown** (per-package prompt) instead of borrowing a clock from a different build. Defense-in-depth behind the fresh-snapshot fix.
- [x] **Test surface** — 22 → 29 tests (4 new in `holds::tests` covering the guard, 3 new in `sync_db::tests` covering `%VERSION%` parsing); [Test Matrix](testing/20260707 - Test Matrix for nog v1-0-5.md) section 18 adds regression-guard checks for the fresh-snapshot path, the fallback warning, and the guard.
- [x] **Dogfooded on the AUR binary (2026-07-08)** — `yay -S nog` install of 1.0.5 reproduced the fix live: a morning v1.0.4 run marked 12 day-old packages "Ready" (up to *"317 days past window"*); the afternoon v1.0.5 run held all first-sighting updates with sane countdowns, matching an independent recomputation from the fresh DBs. [Test Results](testing/20260708 - Test Results for nog v1-0-5.md).

### v1.0.4 — Released
- [x] **Phase 7 — split-PKGBUILD pkgbase coupling** — generalizes v1.0.3's `*-headers` rule to all packages sharing a `pkgbase`. `sync_db.rs` now reads the `%BASE%` field from pacman's sync DBs; `TierManager` consults `PkgbaseIndex` to bucket siblings to the highest tier present in their group. Auto-handles pipewire, mesa, plasma, qt, kde-applications, and every other Arch split PKGBUILD where Arch enforces lockstep via `=` version deps. Closes the 2026-05-25 pipewire-family lockstep failure.
- [x] **Layer B — `lib32-<X>` auto-coupling** — multilib packages have their own pkgbase but are version-pinned to the main package by Arch convention. Stripping `lib32-` and inheriting the base's Tier 1 / Tier 2 tier covers cases like `mesa` ↔ `lib32-mesa` where pkgbase alone wouldn't bridge them. Composes with Layer A — `lib32-libpipewire` correctly resolves Tier 2 via its lib32-pipewire sibling.
- [x] **Layer D — `nog unlock --promote` for any tier** — v1.0.3 restricted unlock to Tier 1. v1.0.4 relaxes it: Tier 2 (15-day hold) and Tier 3 (7-day hold) packages can be promoted too. Necessary fallback if a tier-mismatched lockstep deadlock recurs in a configuration the auto-coupling doesn't catch.
- [x] **Test surface** — 14 → 22 tests (8 new in `tiers::tests`); [Test Matrix](testing/20260525 - Test Matrix for nog v1-0-4.md) section 17 adds 16 regression-guard checks across 17a (pkgbase coupling), 17b (lib32), 17c (live family-upgrade reproduction), 17d (Tier 2 unlock), 17e (no false positives).
- [x] **Dogfood (post-AUR)** — [v1.0.4 Test Results](testing/20260525 - Test Results for nog v1-0-4.md) captured on the AUR-delivered binary (no findings); pkgbase coupling, lib32- rule, and composed Layer A+B all verified live; 22/22 unit tests run in the AUR build's `check()` phase on every install.

### v1.0.3 — Released
- [x] **Phase 6 — tier coupling for headers + DKMS** — `<X>-headers` auto-inherits Tier 1 when `<X>` is Tier 1 (hardcoded, same PKGBUILD → same build date); new optional `[groups]` table in `tier-pins.toml` for non-standard kernel names or custom bundles; plan-time desync detector compares installed kernel vs. headers versions; `nog update --realign` recovers a system already in the desynced state by pulling held kernels forward to match the installed headers; 14/14 tests (8 new in `tiers::tests`); [Test Matrix](testing/20260513 - Test Matrix for nog v1-0-3.md) section 16 with 16 regression-guard checks across 16a/b/c/d
- [x] **`testing/` folder convention adopted** — per-release Test Matrix + Test Results + a nog-specific `RELEASE-CHECKLIST.md` matching the KognogOS ecosystem layout
- [x] **Dogfood (post-AUR)** — [v1.0.3 Test Results](testing/20260513 - Test Results for nog v1-0-3.md) captured on the AUR-delivered binary (no findings); coupling assertions verified live, `cargo test --release --locked` runs 14/14 green on every machine via the PKGBUILD `check()` step

### v1.0 release kit — ✅ Shipped
- [x] **PKGBUILD in tree** at repo root, kept in lockstep with the latest tag
- [x] **AUR submission** — [`ssh://aur@aur.archlinux.org/nog.git`](https://aur.archlinux.org/packages/nog) tracks releases; maintained via `~/Programs/aur-nog-remote/`
- [x] **Dogfood** — full [`Test Matrix`](testing/20260513 - Test Matrix for nog v1-0-3.md) run captured in [`v1.0 Test Results`](testing/20260419 - Test Results for nog v1-0-0.md); the dogfood surfaced the v1.0.1 zstd fix and the v1.0.2 polish batch, both validated on the AUR-delivered binary
- [x] **Release discipline** — every release now runs through local `makepkg -si` test → AUR push → uninstall + fresh AUR install verification

### v1.0 — All phases shipped
- [x] ~~Phase 1 — sync DB reader with gzip + zstd support~~ ✅
- [x] ~~Phase 2 — hold evaluation logic~~ ✅
- [x] ~~Phase 3 — wire into `nog update`~~ ✅
- [x] ~~Phase 4 — AUR helper detection~~ ✅
- [x] ~~Phase 5a — AUR build-date resolution~~ ✅
- [x] ~~Phase 5b — documentation polish (man + help)~~ ✅

### v1.0.0 — Released
- [x] CLI skeleton with all subcommands
- [x] Three-tier classification engine
- [x] Real pacman subprocess integration
- [x] `nog search` with color-coded tier annotations
- [x] System-wide install at `/usr/bin/nog`
- [x] `nog pin` with persistent tier changes to `tier-pins.toml`
- [x] AUR package
- [x] Man page
- [x] **Phase 1 — sync DB reader** — reads every enabled pacman sync database (gzip + zstd), extracts build dates for all packages across all repos
- [x] **Phase 2 — hold evaluation logic** — pure function returning Expired / Holding / Unknown for any package; 6 unit tests; 30/15/7 day windows live in `nog.conf`
- [x] **Phase 3 — wired into `nog update`** — `checkupdates` integration, status-grouped output (Ready / Held / Unknown) with Catppuccin Mocha tier colors, interactive y/N prompt for Unknowns, `manual_signoff` honored as Tier 1 expert-mode toggle, Tier 1 install block removed
- [x] **Phase 4 — AUR helper detection** — auto-detects `yay` / `paru`; AUR pending upgrades fold into the status-grouped output; transactions hand off to the helper for combined `-Syu`; one consistent no-sudo rule; `nog pin` writes via `sudo tee`; root-guard catches `sudo nog` invocations when a helper is configured
- [x] **Phase 5a — AUR build-date resolution** — AUR pending upgrades now get real build dates via the helper's cached metadata (`<helper> -Sai`), parsed to Unix timestamps and fed into the hold evaluator; AUR packages bucket as Ready/Held based on actual dates instead of always Unknown; zero new dependencies, zero new network surface from nog itself
- [x] **Phase 5b — documentation polish (docs)** — full man page rewrite (COMMANDS, TIER SYSTEM, DESCRIPTION, FILES now accurate through v0.12.0 behavior and mention AUR integration); clap help-text refresh (top-level `long_about` + per-subcommand short + long descriptions)

---
