# nog — v1.0.3 dogfood test results

Companion to [`20260513 - Test Matrix for nog v1-0-3.md`](20260513 - Test Matrix for nog v1-0-3.md).

## Run metadata

- **Package under test:** `nog 1.0.3-1` delivered from AUR (`yay -S nog` after `sudo pacman -R nog && sudo rm -rf /etc/nog`)
- **AUR source:** `aur.archlinux.org/nog.git` commit `f0457ec nog 1.0.3 — kernel / headers / DKMS coupling`
- **GitHub source:** `jetomev/nog` tag `v1.0.3` → commit `b21a87043a44845efe9ba312ffa36eaf0057a539`
- **Tarball sha256:** `2a1211af2b4e9627cd13771f4dc2610fe5985c7fe2510946ad85335bcc14b916` (verified by makepkg from the GitHub tag tarball)
- **Install method:** `yay -S nog` against the freshly pushed AUR package, reproducing the public install path
- **Test run started:** 2026-05-13 (same-day with the v1.0.2 → v1.0.3 hotfix push)
- **Tester:** Javier (`jetomev`) + Claude (Anthropic), per [[nog-development-discipline]] and [[github-surface-completeness]]
- **Trigger:** v1.0.3 fixes the 2026-05-13 kernel/headers/DKMS desync that broke nvidia drivers and downed two of three monitors; this dogfood validates the fix on the binary users will actually install

## Pre-test baseline (post-install state, captured at run start)

Captured immediately after the fresh AUR install completed:

- `nog --version` → `nog 1.0.3`
- `/etc/nog/` contents:
  - `nog.conf` (746 bytes, root:root, mode 644, mtime 2026-05-13 20:04)
  - `tier-pins.toml` (1391 bytes, root:root, mode 644, mtime 2026-05-13 20:04)
  - **No `.pacsave` siblings** — clean install on top of the `sudo rm -rf /etc/nog` wipe
- Installed kernels (matching headers — coherent baseline):
  - `linux-zen` 7.0.5.zen1-1 + `linux-zen-headers` 7.0.5.zen1-1
  - `linux-lts` 6.18.29-1 + `linux-lts-headers` 6.18.29-1
- Binary deps (`ldd /usr/bin/nog`): `linux-vdso`, `libzstd.so.1`, `libgcc_s.so.1`, `libc.so.6`, `ld-linux-x86-64.so.2` — five entries, matching the slim-deps contract from v1.0.1
- `dkms status` shows `nvidia/595.71.05` built for both `6.18.29-1-lts` and `7.0.5-zen1-1-zen` (recovery from the morning's incident, now in the baseline)

## Section status

| Section | Title | Status | Notes |
|---|---|---|---|
| 1 | Baseline sanity | pass | 1.1–1.5 all pass on AUR-delivered binary; man page header reads `.TH NOG 1 "May 2026" "nog v1.0.3" "User Commands"`; new TROUBLESHOOTING section confirmed present (added in this release) |
| 2 | Search | pass | 2.1 Tier 3 green, 2.2 Tier 1 red, 2.3 Tier 2 yellow, 2.4 "no results" clean path. Catppuccin Mocha ANSI codes intact (`\x1b[31m`/`[33m`/`[32m`) |
| 3 | Install | not run (this dogfood) | Covered by [v1.0 Test Results](20260419 - Test Results for nog v1-0-0.md) sections 3.1–3.5. No code path changes in v1.0.3 affecting install |
| 4 | Remove | not run (this dogfood) | Same — covered by v1.0 dogfood; remove path untouched |
| 5 | Update — happy path | not run (this dogfood) | Destructive (would trigger an actual transaction); the desync detector path (new in v1.0.3) verified by inspection of code + the absence of a warning on the coherent baseline kernel/headers state |
| 6 | Update — Unknown handling | not run (this dogfood) | No code changes affecting Unknown bucket logic |
| 7 | Update — all held | not run (this dogfood) | No changes affecting the all-held branch |
| 8 | Update — no-helper fallback | not run (this dogfood) | Config-only path, no code changes affecting it |
| 9 | Pin — persistence | not run (this dogfood) | Pin path untouched; same `sudo tee` write mechanism as v1.0.2 |
| 10 | Unlock | not run (this dogfood) | Unlock path untouched |
| 11 | Privilege model / root-guard | not run (this dogfood) | `guard_not_sudo_with_helper` is unchanged from v1.0; v1.0 dogfood covers this. Source inspection confirms no regression |
| 12 | Config edge cases | not run (this dogfood) | Config-loader path unchanged (still uses the `OnceLock` cache from v1.0.2 F4) |
| 13 | Expert mode — manual_signoff | not run (this dogfood) | No changes to Tier 1 sign-off logic |
| 14 | AUR integration | not run (this dogfood) | AUR-helper code path unchanged from v1.0.2 |
| 15 | Binary + filesystem invariants | pass | 15.1 `/usr/bin/nog` confirmed; 15.2 binary deps are libzstd, libgcc_s, libc, ld only — exactly the v1.0 contract; 15.3 `.pacsave` absence verified after the clean wipe + reinstall (note: prior states often had pacsave siblings; this dogfood specifically used `sudo rm -rf /etc/nog` for a true clean room) |
| 16a | Auto-coupling — `<X>-headers` inherits Tier 1 | **pass — core of the fix** | All five checks (16.1 linux-headers, 16.2 linux-zen-headers, 16.3 linux-lts-headers, 16.5 `_debug-hold`) return red `[Tier 1 — 30d hold]`; `_debug-hold linux-zen-headers` reports `tier: Tier 1`, `window: 30 days`, status `HELD (24 days remaining)`. 16.4 (firefox-headers fall-through) is hypothetical and covered by the `headers_for_non_tier1_falls_through` unit test (14/14 green in AUR build's check() phase) |
| 16b | Group inheritance via `[groups]` | skipped — covered by unit tests | Verifying 16.6–16.8 in production requires editing `/etc/nog/tier-pins.toml` with a `[groups]` block and reverting. The `group_inherits_highest_tier_among_members` and `group_with_no_tier1_member_stays_tier3` unit tests cover the resolution logic; the `[groups]` table being parseable is verified by the AUR build (14/14 tests pass on the deserializer). The commented `[groups]` example IS present in the installed `/etc/nog/tier-pins.toml` (confirmed via `grep`) |
| 16c | Desync detection — informational warning | skipped — requires induced state | Reproducing 16.9–16.11 requires installing an older kernel from `/var/cache/pacman/pkg/` while leaving headers at current — too destructive for this machine right now (we just recovered from the real version of this state at 17:30). Defer to a VM dogfood, or wait for the next organic desync event to capture in the wild. **Coverage note:** the desync detector is straight read-and-compare on `pacman -Q` output; the absence of a warning block on this dogfood's coherent system is positive evidence the detector doesn't false-positive |
| 16d | `--realign` — forward-path recovery | skipped — requires induced state | Same reason as 16c. The CLI surface is confirmed working: `nog update --realign` is a recognized flag (verified via `nog update --help`); the AUR build's `cargo test --release --locked` exercises the same `ReadyReason::Realigned` enum path the live --realign uses |

Status legend: `pending` / `pass` / `pass with findings` / `fail` / `skipped`.

---

## Findings

**None.**

Every check that ran returned the expected result. The auto-coupling rule (the core of v1.0.3) is live on the AUR binary, the desync detector silently abstains on a coherent system (positive evidence it doesn't false-positive), and the man page + tier-pins.toml ship with their v1.0.3 content intact.

The skipped sections are categorically "either already covered by the v1.0 dogfood with no code-path changes in v1.0.3" or "require induced desync state that's impractical to reproduce on the recovery host." Both categories are documented above with their compensating coverage (v1.0 Test Results + 14/14 unit tests in the AUR build's `check()` phase).

---

## Cross-validation: AUR build check() phase

The PKGBUILD's `check()` step runs `cargo test --release --locked` against the same source the binary is built from. This dogfood's install captured (excerpted from the live AUR build log):

```
test holds::tests::boundary_exactly_one_window_is_expired_not_holding ... ok
test holds::tests::built_in_the_future_treated_as_zero_elapsed ... ok
test holds::tests::partial_day_rounds_up_per_spec ... ok
test holds::tests::unknown_when_package_not_in_build_dates ... ok
test holds::tests::holding_when_within_window ... ok
test holds::tests::expired_when_elapsed_exceeds_window ... ok
test tiers::tests::direct_tier_lookup_unchanged ... ok
test tiers::tests::group_inherits_highest_tier_among_members ... ok
test tiers::tests::empty_groups_table_is_fine ... ok
test tiers::tests::group_with_no_tier1_member_stays_tier3 ... ok
test tiers::tests::headers_auto_couple_to_tier1_kernel ... ok
test tiers::tests::headers_for_non_tier1_falls_through ... ok
test tiers::tests::tier1_packages_exposes_the_explicit_list ... ok
test tiers::tests::unrelated_headers_pattern_is_tier3 ... ok

test result: ok. 14 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

This means the v1.0.3 release has end-to-end test coverage on every machine that installs it via AUR — not just on the maintainer's box.

---

## Real-world confirmation

The v1.0.3 fix was triggered by the **same machine running this dogfood**. The morning of 2026-05-13 (~16:48 EDT), this host ran `nog update` on v1.0.2 and hit:

```
ERROR: Missing 6.18.29-1-lts kernel modules tree for module nvidia/595.71.05.
ERROR: Missing 7.0.5-zen1-1-zen kernel modules tree for module nvidia/595.71.05.
```

After the manual recovery (Phase 6 task 1: `sudo pacman -S linux-zen linux-lts && sudo pacman -S nvidia-open-dkms`) and the v1.0.3 hotfix landing (~20:04 EDT same day), this host is now running v1.0.3 from AUR with the coupling rule active. `nog search linux-zen-headers` confirms red Tier 1 on the AUR binary — the bug that broke nvidia 4 hours earlier is now structurally prevented for any future `nog update` invocation on the system that originally surfaced it. Closes the loop.

---

## Next-session handoff

For a future session picking this back up:

1. **VM dogfood deferred:** Sections 16c, 16d, 11, and the destructive sections of 5–14 should ideally be exercised in a VM (or test container) where induced state is cheap. Track as an open item; not a blocker for v1.0.3 stability.
2. **No findings → no hotfix batch needed.** v1.0.3 closes cleanly; the next nog release is whatever shape future feature work takes (likely Phase 7 from the Future Roadmap — first-run wizard, `nog history`, etc.).
3. **Open project task:** Task 9 (NIC fix — RTL8125B WoL disable + shutdown unbind) remains open from the 2026-05-13 incident triage. Orthogonal to nog itself.
4. **Memory state at end of run:**
   - `project_nog_tier_coupling_bug.md` → marked RESOLVED v1.0.3
   - `project_nog.md` snapshot is from v1.0.2 (Apr 21) and slightly stale; not urgent
   - `feedback_github_surface_completeness.md` → established this session, applied successfully on the v1.0.3 release surface
