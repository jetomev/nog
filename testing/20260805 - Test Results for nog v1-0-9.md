# nog — v1.0.9 dogfood test results

The **"Ironhold" security cycle**: foreign fence (#2), AUR kill switch (#3), chaotic-aur kill switch (#4), held-table sort (#6). Every feature was field-verified live on the reference machine the same evening it was built — an unusually tight loop even for this project, driven by the active AUR supply-chain incident.

## Run metadata

- **Binary under test:** dev build `target/release/nog` at each phase commit (`a4a6fb4` → `f1139cc` → `a417a0e` → `78e6164`). ⚠️ **The AUR-binary verification leg (makepkg → AUR push → `yay -S` fresh install) is DEFERRED**: the AUR is not accepting pushes (July–Aug 2026 security lockdown, no ETA). It runs the day the freeze lifts, per the release checklist steps 9–11.
- **Unit tests:** 42 → 54 (`cargo test --release --locked`, all green at tag time)
- **Test run:** 2026-08-05, on the KognogOS desktop (Ryzen 7 7700, 31 GiB)
- **Tester:** Javier (`jetomev`) + Claude (Anthropic)
- **Trigger:** [Operation Ironhold](https://github.com/jetomev/KognogOS/blob/main/docs/operation-ironhold.md) Phase A — the response sprint to the July–Aug 2026 AUR attacks, opened by the 2026-08-01 fail-open bypass caught in nog's own CSV run logs.

## The live evidence

**19a — foreign fence** (matrix 19a.1–2): a real 176-update run. Report: 4 Ready / 172 Held / 0 Unknown, then `foreign fence — 19 AUR/local package(s) shielded from the handoff`. The yay handoff honored every ignore; exactly the 4 Ready repo packages (+1 pulled dependency) installed, zero foreign packages touched. Notably, `yay` reported **0 AUR updates** during the run — the exact ambiguous emptiness that caused the Aug-1 bypass — and this time it was structurally irrelevant. 19a.4: the bypass is unit test `fence_replays_the_august_first_bypass`.

**19b — AUR kill switch** (19b.1–3): full round trip. `deactivate aur` wrote `/etc/nog/sources.toml` (sudo tee) and printed the consequence block; the following `nog update` opened with `AUR is DEACTIVATED (kill switch) — official repos only`, printed **no** AUR count line, evaluated 172 official updates normally (all held → clean exit); `activate aur` restored, confirming helper setting `'auto'` untouched in nog.conf.

**19c — chaotic-aur kill switch** (19c.1, 19c.3–4): full round trip on the live `/etc/pacman.conf`. Deactivate: backup `pacman.conf.nog-bak-20260805-162035` created, section marker-commented, refresh synced **core/extra/multilib only**. Activate: second backup `…-162415`, section restored, refresh pulled chaotic-aur as the fourth DB. The forensic gate: `diff /etc/pacman.conf.nog-bak-20260805-162035 /etc/pacman.conf` → **identical** — byte-exact restore proven on the live file, matching the unit-test guarantee (which also covers user comments inside the section, 19c.5).

**19d — held-table sort** (19d.1): verified on a 171-row hold list — 1-day Tier 3 movers on top (`cmake`, `github-cli`, …), the Plasma 6.7.4 block mid-table, `plasma-desktop` at Tier 2 / 14 days, and the Tier 1 kernels + mesa anchoring the bottom at 21–23 days. The table now visualizes the tier gradient. Bonus live catch: `google-chrome` crossed its window between two same-day runs and surfaced as Ready — the sort put the next candidates (`cmake`, `github-cli`, 1 day) right at the top, exactly the "what's coming" read the feature was for.

## Findings

**F-1 (process, fixed in-release): in-tree `PKGBUILD` was stale at 1.0.7.** The v1.0.8 release bumped every surface except the in-tree PKGBUILD (the checklist's "Version sync" list includes it; the audit grep was evidently not run against it). Caught during this release's sync pass; bumped straight to 1.0.9. Guard: the checklist's quick-audit grep does cover it — the lesson is to actually diff the grep output against the expected version, not eyeball it.

**No code findings.** All four features behaved to spec on first live contact.

## Deferred (blocked on the AUR freeze)

- Release-checklist steps 9–11: AUR PKGBUILD push, `makepkg -si` smoke test, fresh `yay -S nog` install, AUR badge verification. The AUR remote (`~/Programs/aur-nog-remote/`) is **staged** at 1.0.9 and ships the day pushes reopen. Tracked in [Operation Ironhold](https://github.com/jetomev/KognogOS/blob/main/docs/operation-ironhold.md) as the sprint's only permitted open item.
