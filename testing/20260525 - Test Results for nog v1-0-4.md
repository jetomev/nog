# nog — v1.0.4 dogfood test results

Companion to [`20260525 - Test Matrix for nog v1-0-4.md`](20260525 - Test Matrix for nog v1-0-4.md).

## Run metadata

- **Package under test:** `nog 1.0.4-1` delivered from AUR (`yay -S nog` after `sudo pacman -R nog && sudo rm -rf /etc/nog`)
- **AUR source:** `aur.archlinux.org/nog.git` commit `8666c52 nog 1.0.4 — split-PKGBUILD pkgbase coupling`
- **GitHub source:** `jetomev/nog` tag `v1.0.4` → commit `814499e release: v1.0.4 prep — PKGBUILD pkgver bump`
- **Tarball sha256:** `bb5f6d7424b21526b8cbd52d6cf611c30b057441b65659292fe64a9ee85b6a10` (verified by makepkg from the GitHub tag tarball)
- **Install method:** `yay -S nog` against the freshly pushed AUR package, reproducing the public install path
- **Test run started:** 2026-05-26 (same-day with the v1.0.3 → v1.0.4 hotfix push)
- **Tester:** Javier (`jetomev`) + Claude (Anthropic), per [[nog-development-discipline]] and [[github-surface-completeness]]
- **Trigger:** v1.0.4 fixes the 2026-05-25 split-PKGBUILD lockstep failure that broke `nog update` on the pipewire family; this dogfood validates the fix on the binary users will actually install, on the exact same host that triggered the bug

## Pre-test baseline (post-install state, captured at run start)

Captured immediately after the fresh AUR install completed:

- `nog --version` → `nog 1.0.4`
- `/etc/nog/` contents:
  - `nog.conf` (746 bytes, root:root, mode 644, mtime 2026-05-26 17:52)
  - `tier-pins.toml` (1391 bytes, root:root, mode 644, mtime 2026-05-26 17:52)
  - **No `.pacsave` siblings** — clean install on top of the `sudo rm -rf /etc/nog` wipe
- Installed pipewire family (still at the unupgraded state from before v1.0.4 — the live test fixture):
  - `pipewire`, `pipewire-pulse`, `libpipewire`, `pipewire-audio`, `pipewire-alsa`, `pipewire-jack`, `gst-plugin-pipewire`, `alsa-card-profiles` all at `1:1.6.5-1`
  - `lib32-pipewire`, `lib32-libpipewire` at `1:1.6.5-1`
- `mesa` at `1:26.1.1-2` (upgraded during the 2026-05-25 workaround); `lib32-mesa` still at `1:26.1.1-1`

## Section status

| Section | Title | Status | Notes |
|---|---|---|---|
| 1 | Baseline sanity | pass | `nog 1.0.4` confirmed; man page header reads `.TH NOG 1 "May 2026" "nog v1.0.4"`; `/etc/nog` clean |
| 2 | Search | not run (this dogfood) | Covered by v1.0 dogfood + v1.0.3 dogfood; no code path changes in v1.0.4 affecting search annotation rendering (only what tier each package is *classified as* — verified in section 17 below) |
| 3–14 | Various existing surfaces | not run (this dogfood) | No v1.0.4 code path changes affecting install / remove / unknown handling / pin / privilege model / config edge cases / expert mode / AUR integration. Covered by [v1.0 Test Results](20260419 - Test Results for nog v1-0-0.md) and [v1.0.3 Test Results](20260513 - Test Results for nog v1-0-3.md) |
| 15 | Binary + filesystem invariants | pass | `/usr/bin/nog` confirmed (file list unchanged from v1.0.3); slim deps contract preserved (no new runtime deps in v1.0.4); 22/22 unit tests in AUR build's `check()` phase |
| 16 | Kernel / headers coupling (v1.0.3) | not run (this dogfood) | Preserved behavior; the new pkgbase coupling supersedes the `*-headers` rule for the standard kernels (kernel + headers share `pkgbase`), but the explicit rule still fires as a separate code path. Covered by `headers_auto_couple_to_tier1_kernel` unit test (still green in v1.0.4). No regressions expected and none observed in classify behavior |
| **17a** | Layer A — pkgbase sibling coupling | **pass — core of the v1.0.4 fix** | `libpipewire` → yellow `[Tier 2 — 15d hold]` (was green Tier 3 in v1.0.3); `pipewire-audio` → Tier 2; `gst-plugin-pipewire` → Tier 2; `_debug-hold libpipewire` reports `tier: Tier 2`, `window: 15 days`, `HELD (12 days remaining)`. Sibling lookup via `%BASE%` from pacman sync DB works correctly against live data |
| **17b** | Layer B — `lib32-<X>` auto-coupling | **pass** | `lib32-mesa` → red `[Tier 1 — 30d hold]` (was green Tier 3 in v1.0.3); demonstrates the multilib lockstep bridge across different pkgbases |
| **17a+B composed** | `lib32-libpipewire` | **pass** | yellow `[Tier 2 — 15d hold]` — proves the recursive resolution composes correctly: `lib32-libpipewire`'s sibling is `lib32-pipewire`, which classifies Tier 2 via Layer B (lib32 stripping to `pipewire`), so `lib32-libpipewire` transitively inherits Tier 2 |
| 17c | Live regression — pipewire family upgrades together | pending user action | The 2026-05-25 incident state was preserved on this host as the live fixture (pipewire family all at `1:1.6.5-1`). With v1.0.4 installed, the *next* `nog update` will bucket all 10 pipewire-family packages identically. The previously broken transaction is structurally prevented. Awaiting Javier running `nog update` to confirm end-to-end |
| **17d** | Layer D — `nog unlock --promote` for any tier | **pass** | `nog unlock pipewire` (no `--promote`) shows the new informational format: `nog: 'pipewire' is Tier 2.` + `Tier 2 (15-day hold by default).` + `nog unlock by itself does nothing — it has no per-session state to toggle.` + `nog unlock pipewire --promote` hint. **NOT** the v1.0.3 refusal (`no unlock needed (only Tier 1 is ever held by policy)`) |
| **17e** | No false positives | **pass** | `htop` still green `[Tier 3 — 7d hold]` — coupling only kicks in when a sibling is tier-pinned. No over-eager classification of unrelated packages |

Status legend: `pending` / `pass` / `pass with findings` / `fail` / `skipped` / `not run (this dogfood)`.

---

## Findings

**None.**

Every check that ran returned the expected result. Layer A (pkgbase coupling), Layer B (lib32- prefix), Layer A+B composition, and Layer D (Tier 2 unlock) all live in production on the AUR-delivered binary. The classification rules apply to **live sync-DB data**, not just synthetic test fixtures.

Skipped sections are all "already covered by prior dogfood with no code-path changes in v1.0.4." The diff is concentrated in `src/sync_db.rs` (parse extension), `src/tiers.rs` (classify chain + PkgbaseIndex), `src/main.rs::debug_hold` (wire pkgbase index), and `src/commands/mod.rs` (load_tiers wire + unlock relaxation). Nothing else touched.

---

## Cross-validation: AUR build check() phase

The PKGBUILD's `check()` step runs `cargo test --release --locked` against the same source the binary is built from. This dogfood's install captured (excerpted from the live AUR build log):

```
test holds::tests::boundary_exactly_one_window_is_expired_not_holding ... ok
test holds::tests::built_in_the_future_treated_as_zero_elapsed ... ok
test holds::tests::expired_when_elapsed_exceeds_window ... ok
test holds::tests::holding_when_within_window ... ok
test holds::tests::unknown_when_package_not_in_build_dates ... ok
test tiers::tests::direct_tier_lookup_unchanged ... ok
test holds::tests::partial_day_rounds_up_per_spec ... ok
test tiers::tests::empty_groups_table_is_fine ... ok
test tiers::tests::empty_pkgbase_index_falls_through_to_tier3 ... ok
test tiers::tests::group_inherits_highest_tier_among_members ... ok
test tiers::tests::group_with_no_tier1_member_stays_tier3 ... ok
test tiers::tests::headers_auto_couple_to_tier1_kernel ... ok
test tiers::tests::headers_for_non_tier1_falls_through ... ok
test tiers::tests::lib32_inherits_tier1_when_base_is_tier1 ... ok
test tiers::tests::lib32_inherits_tier2_when_base_is_tier2 ... ok
test tiers::tests::lib32_of_headers_inherits_via_inner_pattern ... ok
test tiers::tests::lib32_of_pkgbase_sibling_resolves_via_own_multilib_pkgbase ... ok
test tiers::tests::lib32_of_tier3_stays_tier3 ... ok
test tiers::tests::pkgbase_sibling_inherits_tier2_from_base ... ok
test tiers::tests::pkgbase_sibling_with_no_tier_pinned_member_stays_tier3 ... ok
test tiers::tests::tier1_packages_exposes_the_explicit_list ... ok
test tiers::tests::unrelated_headers_pattern_is_tier3 ... ok

test result: ok. 22 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

Every machine that installs v1.0.4 via AUR runs the same 22-test gate. Not just the maintainer's box.

---

## Real-world confirmation

The v1.0.4 fix was triggered by a `nog update` failure on **the same machine running this dogfood**. The morning of 2026-05-25, this host ran `nog update` on v1.0.3 and hit:

```
:: installing libpipewire (1:1.6.5-2) breaks dependency 'libpipewire=1:1.6.5-1' required by pipewire
:: installing libpipewire (1:1.6.5-2) breaks dependency 'libpipewire=1:1.6.5-1' required by pipewire-pulse
```

After the design conversation, v1.0.4 implementation, and AUR push the next day (2026-05-26 ~17:00 EDT), the same host is now running v1.0.4 from AUR with pkgbase coupling active. `nog search libpipewire` confirms yellow Tier 2 on the AUR binary; `nog _debug-hold libpipewire` reports Tier 2 with 15-day window. The previously deadlocked pipewire-family upgrade is structurally prevented from ever recurring on this host.

The pipewire family **on disk is still at the unupgraded `1:1.6.5-1` state** as the live regression fixture. Next `nog update` will bucket all 10 family members identically under Held — the test of 17c will close end-to-end once Javier runs it.

Mirror of the v1.0.3 pattern: same machine, same incident class, fix shipped same-day after the report, validated on the AUR-delivered binary.

---

## Next-session handoff

1. **Live 17c verification:** When Javier next runs `nog update`, paste the bucket output. Expectation: all 10 pipewire-family packages (`pipewire`, `pipewire-pulse`, `libpipewire`, `pipewire-audio`, `pipewire-alsa`, `pipewire-jack`, `gst-plugin-pipewire`, `alsa-card-profiles`, `lib32-pipewire`, `lib32-libpipewire`) appear in the same bucket — all Held (1-2 days remaining since they share build date) or all Ready once the hold expires. No tier-mismatched split.
2. **Optional — clear the 140-package backlog:** Now that v1.0.4 is in place, `nog update` should run the big upgrade successfully; pipewire family stays held in a coherent group while everything else flows. Javier's call whether to run it now or wait for the hold to expire.
3. **No further v1.0.4 scope.** No findings, no hotfix batch needed. The next nog release is whatever shape future feature work takes (likely a roadmap entry: first-run wizard, `nog history`, `nog status`, `nog rollback`).
4. **Memory state at end of run:**
   - `project_nog_pkgbase_coupling.md` → mark RESOLVED v1.0.4
   - `project_nog.md` snapshot is now substantially stale (still says v1.0.2 latest); not urgent but worth refreshing in a future session
   - `feedback_github_surface_completeness.md` → applied successfully on the v1.0.4 release surface (camo cache already serving 1.0.4 — no cache-bust required this release)
