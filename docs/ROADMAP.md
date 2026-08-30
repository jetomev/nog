# nog — full roadmap history

*The README carries the upcoming work and the two most recent releases; everything older lives here, newest-first (the [locked convention](../README.md#roadmap)).*

### v1.4.0 — Released (reboot advice, #9)
- [x] New `reboot` module: probe the running system *after* the handoff, so it reflects what pacman did rather than what nog asked for
- [x] Kernel checked by the presence of `/usr/lib/modules/<uname -r>` — no version string is parsed, because the running kernel and its package spell the same version differently
- [x] NVIDIA checked against `/proc/driver/nvidia/version` — the exact comparison that diagnosed the original incident in seconds
- [x] systemd checked against `systemctl --version`, with the distribution suffix dropped
- [x] Observations marked `verified` and carrying both versions; everything else named as advice, never warned anonymously
- [x] Session components (`mesa`, `xorg-server`, `wayland`, `dbus`) get a logout notice, not a reboot demand
- [x] `*-headers` excluded — never loaded into a running system, and present beside every kernel
- [x] A package nog cleared but pacman did not install produces no notice; installed versions are re-read after the handoff
- [x] Root `PKGBUILD` deleted — it had diverged from the AUR copy on `source`, `sha256sums` and `validpgpkeys` while reporting the same version

### v1.3.1 — Released (soname coupling, #13)
- [x] New `local_db` module reads `/var/lib/pacman/local` for `%PROVIDES%` and `%DEPENDS%` — nog's first look at the installed dependency graph
- [x] `sync_db` parses `%PROVIDES%`, its first multi-value field
- [x] Fourth coupling rule: hold a Ready package that would stop providing a soname something installed still requires
- [x] Sonames matched as whole strings, architecture suffix included — 11 same-arch coexistences and 118 `-32`/`-64` pairs must not couple
- [x] Dependents drawn from all installed packages, so a foreign/AUR package counts; such a row reads `blocked by <pkg>` rather than a false countdown
- [x] Unreadable local database leaves the rule inert — v1.3.0 behaviour exactly
- [x] Tests 86 -> 100

### v1.3.0 — Released (one package manager per source, #10)
- [x] Handoff split: pacman -> AUR helper -> flatpak -> snap, each run by the tool that owns the source
- [x] AUR step is handed cleared package names, not a filtered sysupgrade — same idiom as flatpak and snap
- [x] Foreign fence demoted to a second layer and relabelled; the omission bypass is now closed structurally
- [x] pacman failure cancels the run; later failures report and ask, defaulting to no
- [x] Run log records `installed with incomplete steps: ...` when a run is carried through
- [x] A non-zero handoff is reported as "did not complete", not "failed" — the exit status cannot distinguish a declined prompt from a real error
- [x] Review gate renamed `Begin the handoff?`, so it no longer echoes pacman's own prompt word for word
- [x] Tests 80 -> 86 (aur.rs gains its first test module)

### v1.2.1 — Released (hotfix: family coupling, #11)
- [x] `pkgbase_coupling_demotions()` — packages sharing a `%BASE%` release together
- [x] Demotion pass iterates to a fixpoint (capped at 16) so every rule is transitive
- [x] `cohort_coupling_demotions()` — holds a split `(pkgver -> pkgver)` group of 3+, for families with no metadata to couple on (the Qt6 case)
- [x] Validated by replaying the 08-25 run: 4 true positives, 0 false positives across 221 packages
- [x] Tests 69 -> 80

### v1.2.0 — Released (C2: Snap backend)
- [x] `snap` joins `sources.toml` with `nog activate|deactivate snap`
- [x] Snap updates in the same tables, `· snap` tag, tier windows clocked by the tracked channel's publish date (`snap info`)
- [x] Same naming-based hold enforcement as flatpak, with its own `apply_list()` tests
- [x] `snap refresh` escalates through sudo (snapd requires root) with snap's own progress shown
- [x] Dormant when snapd is absent — never an error (snapd is AUR-only on Arch)

### v1.1.0 — Released (C1: Flatpak backend)
- [x] `flatpak` joins `sources.toml` with `nog activate|deactivate flatpak`
- [x] Flatpak updates folded into `nog update`: own source count, rows in the Ready/Held/Unknown tables, `· flatpak` tag in the Note column
- [x] Tier hold windows applied to flatpak refs via the pending commit's publish date (`flatpak remote-info`)
- [x] Holds enforced by naming (flatpak has no `--ignore`) — pure `apply_list()` with tests proving held/skipped refs can never be applied
- [x] Fail-closed on an unreachable remote; dormant when the flatpak binary is absent
- [x] [#8](https://github.com/jetomev/nog/issues/8) the flatpak handoff shows its own transaction (dogfood finding — "show the work", now a rule for every backend)

### v1.0.9 — Released ("Ironhold" security cycle)

Built during the July–August 2026 AUR supply-chain attacks (malicious orphan adoptions, `-bin` typosquats with sudo-time malware; the AUR froze all pushes on Aug 2), as Phase A of the cross-project [Operation Ironhold](https://github.com/jetomev/KognogOS/blob/main/docs/operation-ironhold.md). Every item was field-verified live on the reference machine the day it was built.

- [x] **The foreign fence ([#2](https://github.com/jetomev/nog/issues/2))** — fixes a fail-open hole caught by nog's own CSV run logs: on 2026-08-01, with the AUR mid-lockdown, `yay -Qua` returned empty, two held AUR packages vanished from the report, and the handoff's own resolution upgraded them anyway. Now every installed foreign package is ignored at handoff unless nog explicitly cleared it **this run** (Ready, or a user-approved Unknown) — "couldn't check" and "all quiet" are treated identically, and holds survive an AUR blackout. The bypass is reproduced as a unit test.
- [x] **AUR kill switch ([#3](https://github.com/jetomev/nog/issues/3))** — `nog deactivate aur` / `nog activate aur`: persisted in the new nog-owned `/etc/nog/sources.toml`, gated upstream of helper detection (helper-agnostic), turns off every AUR-aware path at once; the handoff runs pacman-only. `nog.conf` is never rewritten — the configured helper resumes exactly on activation. Unreadable state fails closed.
- [x] **chaotic-aur kill switch ([#4](https://github.com/jetomev/nog/issues/4))** — `nog deactivate chaotic-aur` / `activate chaotic-aur`: nog comments the `[chaotic-aur]` section in/out of `/etc/pacman.conf` itself (`#nog# ` marker; timestamped backup first; user comments inside the section survive; restore is byte-exact — proven by unit test *and* a live `diff` against the pre-toggle backup), then refreshes the sync DBs. The repo definition is the gate: nothing on the system can resolve from a deactivated chaotic-aur, and installed chaotic packages sit frozen.
- [x] **Held table sorted by days remaining ([#6](https://github.com/jetomev/nog/issues/6))** — soonest-to-release first, ties alphabetical; the table now reads as a release calendar and visualizes the tier gradient (1-day Tier 3 movers on top, 23-day Tier 1 kernels at the bottom). The CSV run log mirrors the same order.
- [x] **Test surface** — 42 → 54 (three fence tests incl. the Aug-1 replay, four `sources` state tests, five pacman.conf toggle tests incl. the byte-exact roundtrip).
- [x] *AUR note: v1.0.9 was released **during** the AUR push freeze, so it shipped from source only. The freeze lifted on 2026-08-14 and the AUR caught up — `nog` is live there at v1.2.0.*

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
