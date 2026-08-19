# nog — full changelog

*The README carries the two most recent entries; the complete history lives here, newest-first.*

### v1.2.0 — August 10, 2026

**C2: nog speaks Snap.** The same shape as the Flatpak backend — snaps are detected, aged through the tier windows (clock = the publish date of the pending revision in the snap's *tracked channel*, read from `snap info`), tagged `· snap` in the tables, and refreshed only when nog has cleared them. Holds are enforced by naming, as with flatpak, and pinned by their own tests.

Two structural differences from flatpak, both deliberate: `snap refresh` requires root, so nog escalates through `sudo` for that step alone (staying unprivileged everywhere else); and because snapd is AUR-only on Arch, its absence is *normal* — the source sits dormant and silent, never an error. Snap is expected to serve the tail: most software is covered long before the chain reaches it, but when it's the only home for something, nog can now manage it like everything else.

Dogfooded live on 2026-08-10 with `hello` installed at an old revision — detection, channel-date resolution (1663 days past window, honestly reported), the source tag, and a real sudo-escalated refresh.

Also in this release: the README's Roadmap and Changelog now keep only the upcoming work and the two most recent releases, with full history in [docs/ROADMAP.md](docs/ROADMAP.md) and [docs/CHANGELOG.md](docs/CHANGELOG.md) — the file had grown to a thousand lines, which serves archaeologists better than newcomers.

### v1.1.0 — August 10, 2026

**C1 of the [v2 multi-source arc](docs/v2-design.md): nog speaks Flatpak.**

Flatpak becomes a first-class source rather than a parallel universe you update separately. `nog update` now queries flatpak alongside pacman and the AUR, reports its own per-source count, and folds every pending app into the same Ready / On-hold / Unknown tables — aged by the same tier windows, with the clock set by the pending remote commit's publish date (the build-date analogue). Rows carry a `· flatpak` marker so the source of every update is visible before you say yes.

Holds work differently under the hood because flatpak has no `--ignore`: nog enforces them **by naming**, passing exactly the refs it cleared this run. That rule lives in one pure function (`flatpak::apply_list`) with tests proving a held or skipped ref can never reach the transaction.

The backend is optional in both directions — a missing `flatpak` binary leaves the source dormant (never an error), and `nog deactivate flatpak` freezes it by choice. A failed remote query is reported and skipped, never assumed quiet (the fail-closed doctrine from v1.0.9).

Dogfooded live on 2026-08-10 across the full pipeline: detection, date resolution, tier bucketing, the source tag, the kill switch, and a real apply. One finding came out of it and shipped in the same release — [#8](https://github.com/jetomev/nog/issues/8): the flatpak handoff now shows flatpak's own transaction instead of a single silent line. *Show the work* is now a requirement for every future backend.

### v1.0.9 — August 5, 2026
**The "Ironhold" security cycle — holds that fail closed, and supply chains you can sever in one command**

Built live during the July–August 2026 AUR supply-chain attacks, as Phase A of [Operation Ironhold](https://github.com/jetomev/KognogOS/blob/main/docs/operation-ironhold.md). The trigger was nog's own CSV run log catching a real bypass on this machine: on Aug 1, with the AUR mid-lockdown, the AUR query silently returned empty, two packages that had been *held with 5 days remaining the night before* vanished from the report — and the handoff upgraded them anyway. (Both proved clean. The hole didn't.)

- 🛡 **The foreign fence ([#2](https://github.com/jetomev/nog/issues/2))** — the handoff's `--ignore` list now always includes **every** installed foreign package except the ones nog explicitly cleared this run (Ready, or a user-approved Unknown). "Couldn't check the AUR" and "no AUR updates" are treated identically — the fence stands either way, so a hold can never again evaporate because a query failed. Healthy runs behave exactly as before (ignoring an up-to-date package is a no-op). The Aug-1 bypass is now a unit test.
- 🔌 **`nog deactivate aur` / `nog activate aur` ([#3](https://github.com/jetomev/nog/issues/3))** — the AUR kill switch. One command turns off every AUR-aware path — update detection, install routing, handoff (pacman-only) — for incident response during an active attack. State persists in the new nog-owned `/etc/nog/sources.toml` (written via `sudo tee`, tier-pins style); your `nog.conf` helper setting is never touched and resumes exactly on activation. An unreadable state file fails **closed**.
- 🔌 **`nog deactivate chaotic-aur` / `activate chaotic-aur` ([#4](https://github.com/jetomev/nog/issues/4))** — the binary-repo kill switch. nog comments the `[chaotic-aur]` section in/out of `/etc/pacman.conf` itself: timestamped backup first, `#nog# ` marker so activation restores exactly and only what nog disabled (your comments inside the section survive), DB refresh after. With the section out, *nothing* on the system — pacman, helpers, libalpm GUIs — can resolve from the repo; installed chaotic packages sit frozen. Restore verified byte-exact live (`diff` against the pre-toggle backup: identical).
- 📅 **Held table sorted by days remaining ([#6](https://github.com/jetomev/nog/issues/6))** — soonest-to-release on top, Tier 1 heavyweights at the bottom: the hold list now reads as a release calendar, and — a happy accident — visualizes the tier gradient itself. The CSV log mirrors the order.
- 📦 **Released during the AUR push freeze** — v1.0.9 could only be installed from source at the time ([context](https://github.com/jetomev/KognogOS/blob/main/docs/operation-ironhold.md)). Arch reopened pushes on 2026-08-14 and every staged package shipped; `yay -S nog` now gets v1.2.0, signature-verified at build time.

Internals: new pure `sources` module (state parse/render + the pacman.conf section toggler) and `holds::foreign_fence()` + `pacman::foreign_package_names()`. Every feature was field-verified on the reference machine the day it was built — including one full 176-update run with the fence live and both kill-switch round trips. Unit tests 42 → 54; warnings unchanged at 7.

### v1.0.8 — July 29, 2026
**CSV run logging — nog remembers every update run**

The follow-through on v1.0.7's closing hint: every `nog update` now writes a history record you can grep, sort, or open in a spreadsheet.

- 🗒 **Per-day CSV log** — `YYYYMMDD nog-update.csv` under `[paths] run_logs` (default `~/.local/share/nog/logs`; leading `~/` expands against `$HOME`). One header line per file; runs on the same day append.
- 🪞 **Faithful mirror** — one row per package with the exact table columns the report showed (`bucket, package, old_version, new_version, tier, note`), snapshotted after the realign and lib32-coupling passes, plus the banner context (`date, time, user`) on every line so files stay self-describing under `cat`/`grep`.
- 🏁 **Outcome column** — how the run ended: `installed`, `cancelled` (the Proceed gate), `all held`, `up to date` (logged as a marker line, so no-op runs still count), or `handoff failed (status N)`. The log answers "did I actually install that day?" — not just "what did nog show me?"
- 🧹 **3-month retention** — files dated older than 90 days are pruned after each successful write, cutoff computed via the system `date` (still no datetime crate).
- 🛟 **Soft-fail discipline** — unwritable directory, missing `date`, failed prune: all warn and continue. Logging never blocks or aborts an update.
- ⚙️ **New config key** — `[paths] run_logs`, optional; existing `nog.conf` files keep working via a serde default. nog runs unprivileged, so the log lives in user space — `/var/log` was never an option.

Internals: new pure `runlog` module (RFC-4180 escaping, rendering, retention decision) mirroring the `format_table()` philosophy — pure core, thin soft-failing IO wrappers. Unit tests 35 → 42; warnings unchanged at 7. Also refreshed the README's `Example: nog update` block, which had missed the v1.0.7 reformat.

### v1.0.7 — July 18, 2026
**`nog update` output, reformatted**

Rebuilds the update report around scannable tables and an at-a-glance header, from a user-driven redesign.

- 🪪 **Header block** — a `nog - Update!` banner with **Date / Time / User**. Date/time come from the system `date` — no datetime-crate dependency (nog's slim dep tree is a feature).
- 🔢 **Per-source counts** — separate lines for the official (pacman) and AUR (helper) update counts, so the split is visible before the detail. Future sources (flatpak, …) slot in as additional lines.
- 📋 **Tables** — Ready / Held / Unknown each render as an aligned table: `Package (N) | Old Version | New Version | Tier | Note`. Empty sections show `(none)`. Tier is a bare, per-tier-colored digit. Terminal width is intentionally ignored — long version strings (e.g. the `gcc` `+g…` builds) simply widen the columns.
- ✋ **Proceed gate** — a new `nog: Proceed with installation? [Y/n]` before handoff. yay/pacman still presents its transaction and asks again — two deliberate review layers so an expert can catch and cancel.
- ✅ **Closing block** — `Update finished!` / `Thank you for using nog!` on success.

Internals: a pure, unit-tested `format_table()` (alignment + the `(none)` case), with the Ready/Held reason text extracted into `ready_note` / `held_note`. Unit tests 33 → 35; warnings unchanged.

The per-run **CSV log + 3-month retention** hinted at in the closing lines lands next, in v1.0.8.

### v1.0.6 — July 15, 2026
**Hotfix — split `lib32`/base pairs could abort the whole transaction**

A `lib32-<X>` multilib package hard-depends on its base `<X>` at an exact version (`lib32-nvidia-utils` → `nvidia-utils=<ver>`). Their hold windows are dated independently from each package's first-sighting date, so they can cross their thresholds on different days and land in **different buckets** — one Ready, one Held. Releasing only half the pair leaves pacman unable to satisfy the exact-version dependency, and it aborts the **entire** `nog update` — taking every other Ready package down with it. Reported in [#1](https://github.com/jetomev/nog/issues/1), hit live on the nvidia stack (`lib32-nvidia-utils` Ready while `nvidia-utils` was Held).

Same *family* as the tier-coupling (v1.0.3) and pkgbase-coupling (v1.0.4) fixes, but a distinct trigger: coupling existed for **tier bucketing**, not for **hold release** — two version-locked packages in the same tier could still be released on different days.

**Fix:**

- 🔗 **Hold-release coupling.** New `holds::lib32_coupling_demotions()` (pure, unit-tested) takes the Ready and Held name sets and returns the Ready packages to demote so each split `lib32`/base pair moves as a unit. Runs as a post-bucketing pass in `nog update`, before the ignore list is built, so a demoted package is genuinely withheld. **Bidirectional** — fires whether the `lib32-` half or the base half is the one still held.
- 🏷 **Named hold reasons.** The Held listing now says *why* a coupled package waits — `[Tier 3 · coupled to nvidia-utils · 4 days]` — inheriting the partner's countdown so both rows clear together.

**Scope:** name-pattern coupling (`lib32-` ↔ base), the reported failure; a fuller version keyed on the real `depends`/`provides` graph is noted in [#1](https://github.com/jetomev/nog/issues/1) for later. Ready↔Held only — a partner in the **Unknown** bucket keeps its per-package prompt.

Unit tests 29 → 33; warnings unchanged.

### v1.0.5 — July 7, 2026
**Hotfix — hold windows dated from stale sync DBs**

Fixes the third — and most fundamental — bug in the hold system's short history: hold windows were being measured from the **wrong package's build date**. Surfaced 2026-07-06 when a routine `nog update` reported `lib32-brotli` as *"975 days past window"* — for a package built **the day before**. Post-mortem showed all 14 "Ready" packages that day were 1–4 days old and belonged in Held; among them `bluez` ("53 days past window", built 2 days earlier) and `qtkeychain-qt6` ("62 days past window", built *that same day*).

Root cause: a **split-brain between two databases.** `nog update` gets its candidate list from `checkupdates`, which syncs fresh DBs into a private dbpath as an unprivileged user. But it read build dates from `/var/lib/pacman/sync` — which only refreshes when root syncs, i.e. during the yay/pacman handoff *after* the hold report. So for any update published since the last run (by definition, every update seen for the first time), the system DB still held the *predecessor* version, and the hold was clocked from the predecessor's builddate. Slow-moving packages (predecessor older than the window) sailed through with zero hold; fast-moving packages landed in Held with a wrong clock that silently self-corrected on later runs — which is why the bug stayed invisible from v1.0.0 until a 2.7-year-stale predecessor made the number absurd. **The Tier 1 implication was the serious one:** a new kernel arriving after a >30-day gap since the previous kernel's build would have skipped its 30-day window entirely.

**Fixes:**

- 📸 **Candidate-fresh snapshot.** `sync_db::load_fresh_packages()` walks the DBs `checkupdates` just synced (`$CHECKUPDATES_DB`, default `${TMPDIR:-/tmp}/checkup-db-<uid>/sync/`) — the exact snapshot that produced the candidate list. `nog update` prefers it and falls back to `/var/lib/pacman/sync` with a visible warning only if the snapshot is missing.
- 🛡 **Candidate-version guard.** `sync_db` now parses `%VERSION%`, and the new `holds::evaluate_candidate()` refuses to date a hold from a DB entry whose version isn't the pending candidate's. Mismatches route to **Unknown** and the per-package y/N prompt — honest about what nog actually knows, instead of trusting a clock borrowed from a different build. AUR entries (helper-provided dates, no version) skip the guard, unchanged.

**What this changes for existing installs:**
- The first `nog update` after upgrading may show a **noticeably shorter Ready list** — brand-new updates that pre-1.0.5 would have skipped their hold now correctly enter **Held** with sane countdowns. This is the protection working for the first time on first-sighting updates.
- "Days remaining" figures on already-Held packages may shift by a few days — they're now measured from the candidate's true builddate.
- The Unknown-bucket copy now also mentions version-mismatched DB entries.

Verified live before release: the fixed binary and an independent recomputation from the fresh DBs agreed on all 22 pending updates (21 repo + 1 AUR), every one correctly Held; unit tests 22 → 29.

### v1.0.4 — May 25, 2026
**Hotfix — split-PKGBUILD pkgbase coupling**

Fixes a regression-class bug in the same architectural class as v1.0.3, surfaced 2026-05-25 when `nog update` produced an unresolvable transaction. `pipewire` and `pipewire-pulse` were Tier 2 with 2 days remaining; the rest of the pipewire family (`libpipewire`, `pipewire-audio`, `pipewire-alsa`, `pipewire-jack`, `gst-plugin-pipewire`, `alsa-card-profiles`, `lib32-pipewire`, `lib32-libpipewire`) defaulted to Tier 3 Ready. pacman aborted:

```
:: installing libpipewire (1:1.6.5-2) breaks dependency 'libpipewire=1:1.6.5-1' required by pipewire
:: installing libpipewire (1:1.6.5-2) breaks dependency 'libpipewire=1:1.6.5-1' required by pipewire-pulse
```

Root cause: Arch's split-PKGBUILD convention ships multiple subpackages from one source (`pkgbase = pipewire` here) and enforces `=` version dependencies between them. Tier-mismatched holds across siblings violate that lockstep. v1.0.3 fixed the special case (`<X>-headers`); v1.0.4 generalizes.

**Fixes:**

- 🔗 **Layer A — pkgbase sibling coupling.** `sync_db.rs::parse_desc` now extracts the `%BASE%` field from every package in the sync DBs and exposes a `load_packages()` API returning rich metadata. A new `PkgbaseIndex` (constructed in `TierManager::with_pkgbase_index`) maps each package to its pkgbase and each pkgbase to its sibling list. `TierManager::classify()` consults the index: when classifying a package P with pkgbase B, the result is the highest tier present among siblings of B. Auto-handles pipewire, plasma, qt, kde-applications, gnome family — anywhere Arch ships coordinated subpackages with versioned deps.
- 🔀 **Layer B — `lib32-<X>` auto-coupling.** Multilib packages have their own PKGBUILD (different pkgbase) but Arch enforces version-pinned lockstep with the main package. The rule strips `lib32-` and inherits the base's tier if Tier 1 or Tier 2. Composes with Layer A: `lib32-libpipewire`'s pkgbase is `lib32-pipewire`, which classifies Tier 2 via the lib32- rule stripping to `pipewire` — so `lib32-libpipewire` correctly inherits Tier 2 transitively.
- 🔓 **Layer D — `nog unlock --promote` for any tier.** v1.0.3 restricted unlock to Tier 1 with the message "no unlock needed (only Tier 1 is ever held by policy)." That assumption was wrong — Tier 2 packages are held within their 15-day window too, and during a tier-mismatched lockstep failure the user needs to release Tier 2 holds to break the deadlock. The rule is now: any package can be force-upgraded via `--promote`, regardless of tier. The informational (no `--promote`) mode now shows tier-specific copy explaining the relevant hold window.

**What this changes for existing installs:**
- After upgrading to v1.0.4, **many more packages will silently re-classify** to Tier 1 or Tier 2 via pkgbase coupling. Examples:
  - pipewire family (`libpipewire`, `pipewire-audio`, etc.) → Tier 2 (inheriting from `pipewire`)
  - lib32-mesa, lib32-vulkan-icd-loader, etc. → Tier 1 (inheriting from `mesa` via lib32 prefix)
  - plasma-meta siblings → Tier 2 (inheriting from `plasma-desktop`)
  - qt5/qt6 sub-libraries → Tier 2 if any qt package is Tier 2
- On the next `nog update`, you'll see **more packages in the Held bucket** than v1.0.3. This is the fix in action — those siblings should never have been able to flow ahead of their base. Same pattern of silent re-tiering as v1.0.3's `*-headers` rule, just broader.
- `nog search` annotations now reflect pkgbase coupling too — `nog search libpipewire` shows yellow `[Tier 2 — 15d hold]` where v1.0.3 showed green Tier 3.

**Performance note:** `load_tiers()` now walks the sync DB (~18k packages on a typical Arch install) once per nog invocation to build the pkgbase index. The walk is OnceLock-cached so repeated classify calls within the same process don't re-walk. Adds a one-time cost (hundreds of ms) to commands that previously didn't touch the DB (`nog install`, `nog search`, `nog pin`, `nog unlock`). Accepted for the correctness gain.

**Tests:** 14 → 22 (8 new in `tiers::tests`):
- `lib32_inherits_tier1_when_base_is_tier1` (e.g., `lib32-mesa` → Tier 1)
- `lib32_inherits_tier2_when_base_is_tier2`
- `lib32_of_tier3_stays_tier3`
- `lib32_of_headers_inherits_via_inner_pattern`
- `pkgbase_sibling_inherits_tier2_from_base` (the pipewire family)
- `pkgbase_sibling_with_no_tier_pinned_member_stays_tier3`
- `empty_pkgbase_index_falls_through_to_tier3` (back-compat with v1.0.3 tier-pin-only behavior)
- `lib32_of_pkgbase_sibling_resolves_via_own_multilib_pkgbase` (composed Layer A + B)

**TEST-MATRIX:** new section 17 with 16 regression-guard checks across 17a (Layer A pkgbase), 17b (Layer B lib32), 17c (live regression — pipewire family upgrades together), 17d (Layer D Tier 2 unlock), 17e (no false positives on coherent systems).

No new dependencies. Same dynamic-libzstd linking contract as v1.0.1/v1.0.2/v1.0.3.

### v1.0.3 — May 13, 2026
**Hotfix — kernel / headers / DKMS coupling**

Fixes a regression-class bug where `nog update` could leave a system unbootable. On 2026-05-13 a user's machine ran `nog update`: the Tier 1 30-day hold on `linux-zen` and `linux-lts` kept the kernel binaries pinned, but `linux-zen-headers`, `linux-lts-headers`, and `nvidia-open-dkms` (all Tier 3 defaults) flowed through. The next DKMS rebuild emitted:

```
ERROR: Missing 6.18.29-1-lts kernel modules tree for module nvidia/595.71.05.
ERROR: Missing 7.0.5-zen1-1-zen kernel modules tree for module nvidia/595.71.05.
```

After reboot the running kernel was the old one, no `nvidia.ko` existed for it either, the GPU was unbound, and the user fell back to a single washed-out monitor on simpledrm framebuffer.

The root cause was architectural: kernel + headers + DKMS modules form a triplet that must move together, but `<X>-headers` packages were defaulting to Tier 3 even when their kernel was Tier 1.

**Fixes:**
- 🔗 **Auto-coupling — `<X>-headers` inherits its kernel's Tier 1.** `TierManager::classify()` now treats any package matching the `<name>-headers` pattern as Tier 1 when `<name>` is Tier 1. Same PKGBUILD produces both, so their build dates match and their holds expire together — coupling guarantees they bucket together at plan time too. Hardcoded, not configurable; the Arch convention is universal and the bug is severe.
- 📦 **New optional `[groups]` table in `tier-pins.toml`.** Escape hatch for non-standard kernel names (`linux-cachyos-cacule-headers`) and for bundling extras (e.g. `linux + nvidia-utils`). Members inherit the highest tier present among any other group member. See the commented example in the default `tier-pins.toml`.
- ⚠ **Plan-time desync detector.** At `nog update`, the installed versions of each Tier 1 kernel and its matching headers are compared via `pacman -Q`. Any mismatch prints a red ⚠ block before the Ready/Held/Unknown buckets, naming each desynced pair and pointing at the recovery flag below.
- 🔧 **New `nog update --realign` flag — forward-path recovery.** When desync is detected and the held kernel's pending upgrade version matches the installed headers version, `--realign` pulls that kernel out of the Held bucket and into Ready with the annotation `[Tier 1 · realigned to match installed headers]`. The subsequent transaction upgrades the kernel to match the headers in one coherent step. For the pathological case where no held kernel matches, the flag prints a clear notice and falls back to the standard plan.

**What this changes for existing installs:**
- After upgrading to v1.0.3, **`linux-headers`, `linux-zen-headers`, `linux-lts-headers`, and `linux-hardened-headers` move silently from Tier 3 to Tier 1**. On the next `nog update`, they will appear under **Held** with 30-day windows where v1.0.2 would have shown them as Ready with 7-day windows. This is the fix in action — those headers should never have been able to flow ahead of their kernel.
- The new `[groups]` table is optional; existing `tier-pins.toml` files without it continue to work unchanged.
- DKMS modules (e.g. `nvidia-open-dkms`) are **not** coupled explicitly — they're downstream victims that succeed automatically once kernel ↔ headers are coherent.

**Tests:** 6 → 14. Eight new unit tests in `tiers::tests` cover direct lookup (regression guard), `*-headers` auto-coupling for Tier 1, non-Tier 1 fall-through, group inheritance (both Tier 1 and Tier 2 / 3 cases), empty groups, and the `tier1_packages()` accessor used by the desync detector.

**TEST-MATRIX:** new section 16 with 16 regression-guard checks across 16a (auto-coupling, dev-build-safe), 16b ([groups]), 16c (desync warning), 16d (--realign recovery). Section 16a runs cleanly against any dev build with no system state changes.

No new dependencies. Same dynamic-libzstd linking contract as v1.0.1/v1.0.2.

### v1.0.2 — April 19, 2026
**Dogfood-surfaced polish batch**

Five small fixes and two matrix refinements, all caught during the end-to-end dogfood of the AUR-installed v1.0.1 binary. See [`v1.0 Test Results`](testing/20260419 - Test Results for nog v1-0-0.md) for the full run — every finding is documented there with observed behavior, severity, and fix rationale.

**Fixes:**
- 🛑 **F5 — graceful exit on missing tier-pins.** `load_tiers()` no longer panics with a Rust-native backtrace hint when `/etc/nog/tier-pins.toml` is unreadable. Clean `eprintln!` + `std::process::exit(1)` with the attempted path in the error message for diagnostic clarity.
- 🗂 **F4 — single-warning config load.** `NogConfig::load_default()` now cached via `OnceLock` — no more duplicate "no nog.conf found" warnings on misconfigured systems, and repeat callers read from the cache instead of re-hitting the filesystem.
- 🔒 **F2 — release binaries no longer embed the maintainer's build path.** The `CARGO_MANIFEST_DIR` dev-fallback branch is gated behind `#[cfg(debug_assertions)]`. Release binaries pass `strings` checks cleanly; dev clones still work as before via `cargo run`. Resolves the `makepkg` `$srcdir` warning.
- 🎨 **F1 — `nog search` tier annotations are now config-aware and consistent.** Tier 1 shows `30d hold` by default (was the misleading `manual sign-off`), flipping to `manual sign-off` only when `tier1 manual_signoff = true`. Tier 3 shows `7d hold` (was `fast-track`). All day counts read from `nog.conf`'s `[holds]` section.
- 📝 **F3 — error messages no longer duplicate "exit status".** Every `eprintln!("... exited with status {}", status)` now uses `status.code().unwrap_or(-1)` so output reads `exited with status 1` instead of `exited with status exit status: 1`.

**Matrix refinements:**
- 📋 **M1** — [`Test Matrix`](testing/20260513 - Test Matrix for nog v1-0-3.md) check 15.3 updated: `.pacsave`/`.pacnew` siblings are expected after any uninstall/reinstall cycle (the PKGBUILD's `backup=` directive intentionally preserves user-modified configs)
- 📋 **M2** — [`Test Matrix`](testing/20260513 - Test Matrix for nog v1-0-3.md) check 3.5 no longer keys the pass criterion on a specific exit code for nonexistent packages — helpers have inconsistent behavior here (yay returns 0 with "nothing to do"; paru may return non-zero)

**No behavior changes** beyond the error-path polish and the search label text. 6/6 hold tests still green. Same zstd-via-pkg-config dynamic-linking contract as v1.0.1.

### v1.0.1 — April 19, 2026
**Hotfix — AUR build failure on fresh environments**
- 🔨 `Cargo.toml`: switch `zstd = "0.13"` to `zstd = { version = "0.13", features = ["pkg-config"] }`. The previous config relied on `zstd-sys`'s bundled static build, which failed to link under Arch's makepkg environment (LLD + `-Wl,--as-needed` + `-nodefaultlibs`) because `zstd-sys` didn't emit the static-library link directive in that toolchain config
- 📚 Now uses system `libzstd` via dynamic linking — zero extra runtime dep (pacman already depends on libzstd, so it's always present on Arch)
- 📄 Man page header + README badge + Cargo.toml + `nog.conf` all bumped to 1.0.1
- ℹ No behavior changes; 6/6 hold tests still green. Caught by the v1.0 dogfood pass — exactly what a dogfood is for.

### v1.0.0 — April 19, 2026
**Initial stable release.**

nog is now a complete tier-aware wrapper for pacman and the common AUR helpers, built and polished across six deliberate phases documented in the entries below. This release declares the core contract stable:

**What nog does**
- Classifies every package into Tier 1 (kernel / bootloader / glibc / systemd / mesa — 30-day hold), Tier 2 (DE and key applications — 15-day hold), or Tier 3 (everything else — 7-day hold)
- Computes a full tier-aware upgrade plan before any transaction runs, grouping pending updates into **Ready**, **Held**, and **Unknown** buckets with Catppuccin Mocha tier colors
- Resolves build dates from every enabled pacman sync database (gzip + zstd), then falls back to the configured AUR helper's cached metadata (`<helper> -Sai`) for AUR-only packages — so AUR upgrades get real hold evaluation, not always-Unknown
- Hands off the final transaction to pacman or the helper with `--ignore=<held + skipped>` — pacman-native enforcement, no shadowing
- Escalates to root only via `sudo pacman` for transactions and `sudo tee` for writing `/etc/nog/tier-pins.toml`. Run `nog` as your user — never with `sudo`. The one-rule privilege model is documented exhaustively in the [Privilege model](#privilege-model--what-nog-touches-and-when) section.

**What nog doesn't do**
- Does not shadow, patch, or replace pacman — every transaction goes through pacman's signature verification
- Does not modify any system file outside `/etc/nog/tier-pins.toml`
- Does not make direct network calls — the helper owns all AUR network I/O
- Does not install, upgrade, or remove anything without pacman's own confirmation prompts
- Does not gate explicit user commands — `nog install linux-lts` always proceeds; tier protection lives in the passive `update` path

**Ecosystem**
nog is the native package manager for [KognogOS](https://github.com/jetomev/KognogOS), with a TUI companion ([nogforge](https://github.com/jetomev/nogforge)) and bootloader/terminal utilities ([grubforge](https://github.com/jetomev/grubforge), [alacrittyforge](https://github.com/jetomev/alacrittyforge)).

**Known limitations carried into v1.0**
- AUR build-date resolution depends on the helper's cached metadata being fresh. If the cache is stale, hold windows are evaluated against the cached date rather than live upstream data. Running `<helper> -Sy` (or `yay -Syy`) refreshes it.
- Tier pinning of AUR packages works, but AUR packages without a `Last Modified` field still fall into the Unknown bucket and trigger the y/N prompt.

**Thanks**
Development happened in deliberate phases (see below). Every phase closed with a tagged pre-release and a working dev build; the v1.0.0 tag is the moment the release kit (AUR submission + dogfood) begins.

### v0.12.0 — April 18, 2026
**Phase 5b (docs) — man page and help-text accuracy pass**
- 📜 Full man page rewrite: **DESCRIPTION** updated (30/15/7 day windows, AUR helper mention, expert-mode pointer); **COMMANDS** updated for every subcommand's real v0.12.0 behavior (no more stale "Tier 1 blocked" on install, accurate `nog update` bucketing description, `nog unlock` new semantics); **TIER SYSTEM** rewritten with auto-release default + expert mode; **FILES** now lists sync DBs and pacman.conf as read paths and notes `sudo tee` for tier-pins writes
- 🏷 `man nog` header bumped to `v0.12.0`
- 💬 Clap help text refresh — top-level `long_about` now summarizes the tier system and no-sudo rule in a few sentences; every subcommand (`install`, `remove`, `update`, `search`, `pin`, `unlock`) has a short description for the command list plus a longer one shown in `<cmd> --help`
- 🗂 Roadmap split Phase 5's polish work: screenshots + v1.0.0 CHANGELOG consolidation moved into the **v1.0 dogfood + release kit** step (more honest framing — they belong at release time, not pre-release)
- ℹ No behavior changes; no test regressions (6/6 still green); warnings unchanged at 7

### v0.11.0 — April 18, 2026
**Phase 5a — AUR build-date resolution (the last Unknown falls)**
- 📅 AUR pending upgrades now get real Unix-timestamp build dates by parsing the `Last Modified` field from the helper's cached metadata (`<helper> -Sai`) — no direct AUR RPC calls from nog
- 🧮 The hold evaluator sees a unified build-date map (sync-DB ∪ AUR) and buckets AUR packages as **Ready** or **Held** based on their actual dates, with countdown/past-window reasons identical to official repo packages
- 🧩 New `aur::build_dates_for(helper, packages)` — batched `-Sai` subprocess call, robust colon-split parser that tolerates variable column widths across yay/paru, Unix-timestamp conversion via `date -d`
- 🛟 **Soft-fail discipline preserved** — if the helper is unreachable, the `Last Modified` line is missing, or `date` can't parse the string, those packages fall back to the Unknown bucket and hit the existing y/N prompt. No hard errors, no crashes, no change to current user-facing error paths
- 🔒 **Zero new dependencies, zero new network surface from nog itself** — threat model identical to v0.10.0: nog spawns subprocesses, the helper owns all AUR network I/O
- 🗣 Unknown-bucket message updated — "no resolvable build date" is more accurate than "no build date in any sync DB" now that lookup has multiple paths
- ⚠ Only truly orphan packages (locally-built, disabled-repo, AUR query failure) reach the prompt now — most previous "Unknown" cases resolve automatically

### v0.10.0 — April 18, 2026
**Phase 4 — AUR helper integration + unified no-sudo privilege model**
- 🧩 New `aur` module — helper detection (`yay` → `paru` → `none`) driven by `[aur] helper` in `nog.conf`. Supports `"auto"`, `"yay"`, `"paru"`, `"none"`; hard-errors if the user requests a specific helper that isn't installed
- 📦 `nog update` folds AUR pending upgrades (`<helper> -Qua`) into the existing status-grouped output alongside official repo packages from `checkupdates`. AUR packages bucket as Unknown for now (no sync-DB build date); the y/N prompt already handles them correctly
- 🔄 `nog update` transaction handoff routes through the helper when configured (`<helper> -Syu --ignore=...`) for a single combined official+AUR upgrade. Without a helper, pacman handoff is unchanged
- 📥 `nog install <pkg>` routes through the helper when configured, so AUR-only packages "just work" without a pre-check. The helper resolves sync repos before AUR automatically
- 🔓 `nog unlock --promote` similarly routes through the helper when configured
- 🧑 **No-sudo rule** — single consistent UX: run `nog` as your user. `pacman.rs` now invokes `sudo pacman` internally; `tiers::pin_package` writes `/etc/nog/tier-pins.toml` via `sudo tee`. `nog pin` no longer needs shell-level sudo. Fully backwards-compatible: `sudo nog <cmd>` still works for non-helper paths (sudo-as-root passes through)
- 🛑 **Root-guard** — if nog is invoked via sudo (detected via `$SUDO_USER`/`$SUDO_UID`) *and* a helper is configured, it exits with a clear message pointing the user to drop the `sudo`. Necessary because `yay`/`paru` refuse to run as root
- 📖 **New "Privilege model" section in README** — documents exactly where nog escalates (`sudo pacman`, `sudo tee /etc/nog/tier-pins.toml`), which files it reads without elevation, the single file it ever writes, and the comprehensive list of system files it never touches (pacman.conf, pacman.d, /var/lib/pacman/local, keyring, sudoers, etc.)
- 📜 Man page gains a targeted **PRIVILEGES AND SUDO** section mirroring the README content; version header bumped to 0.10.0; EXAMPLES dropped their `sudo` prefixes. Full man page rewrite (command descriptions, tier metadata) deferred to Phase 5 polish
- ℹ No regressions in existing behavior: 6/6 hold tests still green, 7 warnings (unchanged since Phase 3)

### v0.9.0 — April 18, 2026
**Phase 3 — wired into `nog update` (the tier system goes live)**
- 🔌 `nog update` now calls `checkupdates` (pacman-contrib) to list pending upgrades *without* the `-Sy` side effect, then classifies every pending package against its tier's hold window
- 📊 **Status-grouped output**: three labelled buckets — `Ready to install`, `Held`, `Unknown` — each showing package name, version bump, tier, and either "N days past window", "N days remaining", or "no build date in sync DB"
- 🎨 Tier-colored output using the **Catppuccin Mocha** palette (Tier 1 red `#F38BA8`, Tier 2 yellow `#F9E2AF`, Tier 3 green `#A6E3A1`) — muted subtext color `#A6ADC8` for version/metadata
- ❓ Interactive `[y/N]` prompt per Unknown package (AUR-only, locally-built, or disabled-repo); EOF / non-TTY stdin auto-skips remaining Unknowns with a warning instead of hanging
- 🎚 **Tier 1 policy change, novice-friendly default:** `manual_signoff` now defaults to `false` — Tier 1 auto-updates once the 30-day hold expires. Expert users can set `manual_signoff = true` to restore always-held-until-promoted behavior
- 🔓 `nog unlock <pkg> --promote` kept as the expert-mode escape hatch: force-upgrade a held Tier 1 package right now, bypassing the hold and `manual_signoff`
- 🗑 **Tier 1 install block removed** — `nog install linux-lts` now proceeds normally; tier classification is shown as informational output only. Explicit user commands execute user intent; tier protection lives in the passive update path
- 🧹 `nog unlock` without `--promote` now honestly reports it has no session state to toggle, and points the user at `--promote` for the real action
- ⚠ Warnings reduced to 7 — previously-unused `is_manual_signoff` method is now live; the orphaned `tier1_packages()` helper was removed

### v0.8.0 — April 18, 2026
**Phase 2 — Hold evaluation logic (the date-math engine)**
- 🧮 New `holds` module with a pure `evaluate()` function — given a package, tier, build-date map, and hold config, returns one of `Expired { days_past_window }`, `Holding { days_remaining }`, or `Unknown`
- ✅ 6 unit tests covering all three states, the exact-window boundary, partial-day rounding (ceiling per spec), and future-dated-package edge cases
- 🔒 All inputs explicit including `now: SystemTime` — tests run deterministically, no hidden clock dependency
- 🗓 **New hold spec live in `nog.conf`:** Tier 1 = 30 days, Tier 2 = 15 days, Tier 3 = 7 days
- 🧹 Removed obsolete `hold_days` field from `tier-pins.toml` — hold durations now owned exclusively by `nog.conf [holds]` (single source of truth)
- 🔧 `tiers.rs` cleanup: dropped `hold_days` field and method, simplified `Display` for `Tier` enum, removed unused `std::path::Path` import
- 🧪 Hidden `_debug-hold <package>` subcommand added for internal verification — classifies, looks up build date, evaluates hold, prints result
- ⚠ Warnings reduced from 11 to 9 — previously-unused `HoldsConfig` fields are now active
- ℹ This phase adds no user-visible commands. The `_debug-hold` tool is hidden from `--help`. Phase 3 will wire this evaluator into `nog update`.

### v0.7.0 — April 18, 2026
**Phase 1 — Sync DB reader (foundation for date-based holds)**
- 🧱 New `sync_db` module reads every enabled pacman sync database and builds a map of package → build-date Unix timestamp
- 🗜 Auto-detects **gzip** (core, extra, multilib) and **zstd** (Chaotic-AUR and similar) compression via magic-byte sniffing
- 📋 Respects `pacman.conf` repo priority — first repo wins on name collisions, matching pacman's own resolution
- 🛡 Graceful fallback when `pacman.conf` is unreadable — scans the sync directory directly
- 📦 Indexes **18,000+** packages across all enabled repos on a standard Arch install
- 🧪 Verified against `pacman -Si` output for official and Chaotic-AUR packages — exact timestamp match
- ➕ Dependencies added: `flate2`, `tar`, `zstd`
- 🔢 Version bumped to 0.7.0 to mark v1.0 development in progress
- ℹ This phase adds no user-visible commands. It is infrastructure for Phase 2 and onward.

### v0.6.0 — April 7, 2026
**AUR package + man page**
- 📦 `nog` available on the AUR — install with `yay -S nog`
- 📖 Man page added — `nog.1` installed to `/usr/share/man/man1/`
- 🔢 Version now reads from `CARGO_PKG_VERSION` — no hardcoded strings
- 📋 PKGBUILD installs binary, config files, license, and man page

### v0.5.0 — April 5, 2026
**`nog pin` — persistent tier changes**
- 📌 `nog pin <package> --tier=<1|2|3>` writes changes to `/etc/nog/tier-pins.toml`
- ➕ Pinning to Tier 1 or 2 adds the package to the correct section
- ➖ Pinning to Tier 3 removes it from Tier 1/2 — Tier 3 is the default, no entry needed
- ♻ Changes survive reboots and are immediately reflected in `nog search` annotations

### v0.4.0 — April 5, 2026
**`nog update` — Tier 1 properly excluded**
- 🛡 `nog update` passes Tier 1 packages to pacman via `--ignore` flags
- 🔒 Tier 1 packages are genuinely untouchable during a system upgrade
- ✅ Confirmed: system upgraded 14 packages, zero Tier 1 packages touched

### v0.3.0 — April 4, 2026
**`nog search` + system install**
- 🎨 `nog search` shows color-coded tier annotations for every result
- 📂 Installed system-wide with config files at `/etc/nog/`
- 🚀 `nog` callable from anywhere on the system without a path

### v0.2.0 — March 25, 2026
**Tier system + real pacman calls**
- 🎚 Three-tier classification engine fully implemented in `tiers.rs`
- 📋 `tier-pins.toml` defines all Tier 1/2/3 package assignments
- 🔌 `pacman.rs` wires real subprocess calls — nog installs, removes, and updates for real
- ⛔ `nog install` blocks Tier 1 packages with a clear error message
- 🔓 `nog unlock --promote` allows manual Tier 1 upgrades
- ⚙ `config.rs` reads `/etc/nog/nog.conf` with graceful fallback

### v0.1.0 — March 25, 2026
**Initial release — nog CLI skeleton**
- 🦀 Rust CLI using clap with derive macros
- 📝 All subcommands defined: install, remove, update, search, pin, unlock
- 🏗 Three-tier architecture designed and stubbed

---
