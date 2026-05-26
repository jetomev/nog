# nog — v1.0.4 release test matrix

End-to-end verification checklist for every `nog` command path and behavior. Designed to be run on a **clean Arch system after installing the v1.0 AUR package**, but most tests also work against a dev build (`cargo build --release`).

## How to run

1. Work top to bottom; each section assumes the previous passed
2. Tick the checkbox (`[x]`) next to each test as you verify it
3. If a test fails, stop and file a bug — don't continue past a broken section
4. Between "Update" tests that change system state, either revert with pacman or skip — documented per-test

Conventions:
- `$` — run as your regular user
- `# <- hypothetical` — a command you'd run only if the precondition doesn't already hold
- **EXPECT:** — the observable outcome that makes the test pass

---

## 1. Baseline sanity

- [ ] **1.1** `$ nog --version` prints `nog 1.0.4`
- [ ] **1.2** `$ nog --help` lists all subcommands (`install`, `remove`, `update`, `search`, `pin`, `unlock`) and neither of the hidden `_debug-*` commands
- [ ] **1.3** `$ man nog` opens cleanly; header reads `nog v1.0.4`; **PRIVILEGES AND SUDO** section present
- [ ] **1.4** `/etc/nog/nog.conf` exists, owned by `root:root`, mode 644; `[holds]` = 30/15/7; `[aur] helper = "auto"`
- [ ] **1.5** `/etc/nog/tier-pins.toml` exists, owned by `root:root`, mode 644; `[tier1] manual_signoff = false`

---

## 2. Search

- [ ] **2.1** `$ nog search htop` — shows green `[Tier 3 …]` annotation
- [ ] **2.2** `$ nog search linux-zen` — shows red `[Tier 1 …]` annotation
- [ ] **2.3** `$ nog search firefox` — shows yellow `[Tier 2 …]` annotation
- [ ] **2.4** `$ nog search` with no results prints a clean "no results" message (no trace/panic)

---

## 3. Install

> Requires a helper configured (default `auto` picks `yay` if present). Run as user; do **not** prefix `sudo`.

- [ ] **3.1** `$ nog install htop` — Tier 3 info line printed; helper invoked; package installs; sudo prompt appears exactly once for the pacman step
- [ ] **3.2** `$ nog install linux-lts` — Tier 1 info line printed ("critical system package, will be protected by 30-day hold on future updates"); package installs cleanly; **no block, no `unlock` requirement**
- [ ] **3.3** `$ nog install <AUR-only-package>` (pick any from AUR that isn't in sync repos) — helper builds + installs from AUR; nog shows the tier classification (Tier 3 if not pinned)
- [ ] **3.4** `$ nog install htop nano` — multi-package install works; tier info printed once per package; single transaction
- [ ] **3.5** `$ nog install bogus-nonexistent-xyz` — helper reports "package not found" with a clear message; **no phantom sudo prompt**; nog doesn't falsely claim a successful install. Exit status is helper-specific (yay returns 0 with "nothing to do"; paru may return non-zero) — don't key the pass on a specific code.

---

## 4. Remove

- [ ] **4.1** `$ nog remove htop` — sudo prompt appears; package removed; clean exit
- [ ] **4.2** `$ nog remove bogus-nonexistent-xyz` — pacman reports "target not found"; nog exits with non-zero status
- [ ] **4.3** Removing works uniformly for AUR-installed and official packages (e.g., remove the package from 3.3 — no special handling needed)

---

## 5. Update — happy path

> For these tests, start from a system with **pending updates** available. `checkupdates` should report > 0 packages. If it doesn't, `sudo pacman -Sy` once to refresh, or wait for upstream changes.

- [ ] **5.1** `$ nog update` — "nog: checking for pending updates..." printed
- [ ] **5.2** If yay/paru is configured: "nog: N AUR update(s) reported by <helper>." appears when AUR has pending
- [ ] **5.3** **Ready to install** bucket rendered with green Tier 3 rows; each row shows `name oldver -> newver  [Tier N · X days past window]`
- [ ] **5.4** Colors match Catppuccin Mocha: Tier 1 red `#F38BA8`, Tier 2 yellow `#F9E2AF`, Tier 3 green `#A6E3A1`
- [ ] **5.5** If a recently-built package exists, **Held** bucket renders with "`N days remaining`" and the correct tier color
- [ ] **5.6** AUR packages with no sync-DB entry appear under **Unknown** with `[Tier N · no build date in sync DB]`
- [ ] **5.7** Handoff line printed: `nog: handing off to yay...` (or `paru`/`pacman` depending on config)
- [ ] **5.8** Helper proceeds; sudo prompt appears when pacman step starts; transaction completes successfully
- [ ] **5.9** Re-run `$ nog update` — "nog: system is up to date — nothing to do." No prompts, clean exit

---

## 6. Update — Unknown handling

- [ ] **6.1** With at least one AUR pending upgrade: `$ nog update` prompts `update anyway? [y/N]` for each Unknown; default (Enter) skips
- [ ] **6.2** Typing `y` at the prompt includes the package in the transaction
- [ ] **6.3** Typing `n` or Enter excludes it (shows up in `--ignore` list passed to helper)
- [ ] **6.4** Typing gibberish re-prompts until valid input
- [ ] **6.5** `$ nog update < /dev/null` with Unknowns pending — auto-skip fires with "no interactive input available — skipping remaining unknowns"; no hang

---

## 7. Update — all held

- [ ] **7.1** With `tier1 manual_signoff = true` set (edit `/etc/nog/tier-pins.toml` manually first, then `nog update`): every Tier 1 update lands in **Held** with reason "manual sign-off required — run `nog unlock` to release"
- [ ] **7.2** If all pending items are held: "nog: nothing to install right now — all pending updates are held." No handoff to pacman/helper
- [ ] **7.3** Revert `manual_signoff` to `false` when done

---

## 8. Update — no-helper fallback

- [ ] **8.1** Set `[aur] helper = "none"` in `/etc/nog/nog.conf`
- [ ] **8.2** `$ nog update` — no "AUR update(s) reported" line; only official repo packages show up
- [ ] **8.3** Handoff line reads `nog: handing off to pacman...`; `sudo pacman -Syu --ignore=…` is invoked
- [ ] **8.4** Revert to `helper = "auto"` when done

---

## 9. Pin — persistence and correctness

- [ ] **9.1** `$ nog pin htop --tier=2` — sudo prompt (once); message confirms write to `/etc/nog/tier-pins.toml`
- [ ] **9.2** Inspect the file — `"htop"` now appears in `[tier2] packages`
- [ ] **9.3** `$ nog search htop` — annotation now shows `[Tier 2 …]` yellow
- [ ] **9.4** `$ nog pin htop --tier=1` — moves it; file shows in `[tier1]`; **no leftover** in `[tier2]`
- [ ] **9.5** `$ nog pin htop --tier=3` — removed from tier1; file no longer lists it (Tier 3 is implicit default)
- [ ] **9.6** Reboot (optional) — classification persists
- [ ] **9.7** `$ nog pin foo --tier=7` — error message: "Invalid tier: 7. Must be 1, 2, or 3."

---

## 10. Unlock

- [ ] **10.1** `$ nog unlock htop` — "no unlock needed (only Tier 1 is ever held by policy)" since htop is Tier 3
- [ ] **10.2** `$ nog unlock linux` — explanation printed; pointer to `nog unlock linux --promote`; **no action taken**
- [ ] **10.3** `$ nog unlock linux --promote` — forces an upgrade; sudo prompt; helper or pacman runs; linux is upgraded regardless of hold
- [ ] **10.4** `$ nog unlock linux --promote` when linux is already at latest version — "nothing to do" clean path

---

## 11. Privilege model / root-guard

- [ ] **11.1** `$ sudo nog update` with a helper configured — exits with: "detected `sudo nog` invocation with an AUR helper configured..."; **no partial transaction starts**
- [ ] **11.2** `$ sudo nog install htop` with a helper configured — same guard fires
- [ ] **11.3** Set `[aur] helper = "none"` — `$ sudo nog update` runs to completion (backwards-compat sudo-as-root passthrough)
- [ ] **11.4** `/etc/nog/tier-pins.toml` after a `nog pin` — still owned by `root:root`, mode 644; contents match what was written
- [ ] **11.5** `strace -f -e execve nog install htop 2>&1 | grep sudo` (optional forensic) — confirms exactly one `sudo` invocation (for the pacman step via helper)

---

## 12. Config edge cases

- [ ] **12.1** Delete the `[aur]` section from `/etc/nog/nog.conf` — `nog update` still works, falls back to `helper = "auto"` default
- [ ] **12.2** Set `[aur] helper = "paru"` when paru is NOT installed — `$ nog update` errors cleanly: "nog.conf requests `helper = \"paru\"` but paru is not on PATH"
- [ ] **12.3** Set `[aur] helper = "xyzzy"` — errors: "invalid `[aur] helper` value 'xyzzy'. Expected one of: auto, yay, paru, none"
- [ ] **12.4** Rename `/etc/nog/nog.conf` temporarily; run `$ nog update` — prints "no nog.conf found — using built-in defaults"; still works via dev fallback or bundled defaults
- [ ] **12.5** Restore the file after 12.4

---

## 13. Expert mode — manual_signoff

- [ ] **13.1** Set `[tier1] manual_signoff = true` in `/etc/nog/tier-pins.toml`
- [ ] **13.2** `$ nog update` with a pending Tier 1 upgrade — row lands in **Held** with "manual sign-off required" regardless of how far past the 30-day window it is
- [ ] **13.3** `$ nog unlock linux --promote` still force-upgrades through the sign-off
- [ ] **13.4** Revert to `manual_signoff = false` when done

---

## 14. AUR integration — deep tests

- [ ] **14.1** Install an AUR-only package: `$ nog install <aur-pkg>` — classifies as Tier 3, helper builds
- [ ] **14.2** Wait for an AUR update upstream; `$ nog update` picks it up via `<helper> -Qua`; bucketed as Unknown
- [ ] **14.3** Accept it via y prompt — helper builds + installs; pacman step works; tier info preserved
- [ ] **14.4** Pin that AUR package to Tier 2: `$ nog pin <aur-pkg> --tier=2` — future updates still bucket as Unknown (no sync-DB date) but the tier annotation on the row is now yellow Tier 2

---

## 15. Binary + filesystem invariants

- [ ] **15.1** `$ which nog` → `/usr/bin/nog`
- [ ] **15.2** Binary is single-file, no runtime dependencies other than dynamic libc, checkupdates, and optionally yay/paru
- [ ] **15.3** `$ ls /etc/nog/` → `nog.conf` and `tier-pins.toml` must be present. `.pacnew`/`.pacsave` siblings (e.g., `nog.conf.pacsave`) are **expected** after any uninstall/reinstall cycle — the PKGBUILD's `backup=` directive intentionally preserves user-modified configs. The pass criterion is that no files were created by `nog` itself outside those two configs, not that the directory contains exactly two entries.
- [ ] **15.4** nog has created no files anywhere else during normal operation (check `find / -name "nog*" -newer /etc/nog/nog.conf 2>/dev/null` if paranoid)

---

## 16. Kernel / headers coupling (v1.0.3)

> Regression guard for the 2026-05-13 nvidia driver breakage: `linux-zen` was held by the 30-day Tier 1 window while `linux-zen-headers` (defaulting to Tier 3) and `nvidia-open-dkms` were allowed through. The next DKMS rebuild failed with `Missing 7.0.5-zen1-1-zen kernel modules tree for module nvidia/595.71.05`, the GPU was unbound, and `/usr/lib/modules/<KVER>` was absent after reboot.
>
> v1.0.3 fixes this by treating `<X>-headers` as the same tier as `<X>` when `<X>` is Tier 1, and adds a plan-time desync detector + `--realign` recovery flag.

### 16a. Auto-coupling — `<X>-headers` inherits Tier 1

Non-destructive; safe against a dev build.

- [ ] **16.1** `$ nog search linux-headers` — annotation is red `[Tier 1 — 30d hold]` (previously green Tier 3 in v1.0.2)
- [ ] **16.2** `$ nog search linux-zen-headers` — red `[Tier 1 — 30d hold]`
- [ ] **16.3** `$ nog search linux-lts-headers` — red `[Tier 1 — 30d hold]`
- [ ] **16.4** `$ nog search firefox-headers` (hypothetical) — green `[Tier 3 — 7d hold]` (the rule only fires when the base name is in Tier 1)
- [ ] **16.5** `$ nog _debug-hold linux-zen-headers` — `tier: Tier 1`, `window: 30 days`

### 16b. Group inheritance via `[groups]`

Requires editing `/etc/nog/tier-pins.toml` (revert after).

- [ ] **16.6** Append a `[groups]` table with `custom = ["linux", "weirdname-extras"]` to `/etc/nog/tier-pins.toml`
- [ ] **16.7** `$ nog search weirdname-extras` — annotation is red `[Tier 1 — 30d hold]` (inherited via the group, even though the name doesn't match the `-headers` pattern)
- [ ] **16.8** Remove the `[groups]` table — `$ nog search weirdname-extras` returns to green Tier 3

### 16c. Desync detection — informational warning

Requires an actual installed-version mismatch between a kernel and its headers. The natural way to reproduce: hold a kernel through the v1.0.2-style bug. Easier: forcibly install an older kernel from `/var/cache/pacman/pkg/` while leaving headers at current.

```
# In a VM or test box only:
sudo pacman -U /var/cache/pacman/pkg/linux-zen-<older-version>.pkg.tar.zst
nog update
```

- [ ] **16.9** When kernel and headers versions differ, `nog update` prints a red ⚠ warning block before the Ready/Held/Unknown buckets, listing each desynced pair with both installed versions
- [ ] **16.10** Without `--realign`, the warning includes the recovery hint `nog update --realign`
- [ ] **16.11** No false positive: in a coherent system (kernel and headers at matching versions), no warning appears

### 16d. `--realign` — forward-path recovery

Requires the desync state from 16c.

- [ ] **16.12** `$ nog update --realign` — for each desynced kernel whose pending upgrade version matches the installed headers version, the row moves from **Held** to **Ready** with annotation `[Tier 1 · realigned to match installed headers]`
- [ ] **16.13** A line is printed in the warning block: `--realign: <kernel> <oldver> → <newver> pulled into Ready.`
- [ ] **16.14** Transaction proceeds; the realigned kernel is upgraded; subsequent DKMS rebuild succeeds (e.g., `dkms status` shows nvidia modules built for the new KVER)
- [ ] **16.15** If no held kernel matches the headers version (e.g., headers are ahead of any pending kernel upgrade — pathological case), `--realign` prints `--realign: no held kernel matches the installed headers version` and falls back to the standard plan
- [ ] **16.16** `$ nog update --realign` against a coherent system (no desync) is a no-op for the realign path — runs identically to plain `nog update`

---

## 17. Split-PKGBUILD pkgbase coupling (v1.0.4)

> Regression guard for the 2026-05-25 pipewire-family lockstep failure. `pipewire` (Tier 2) and `pipewire-pulse` (Tier 2) were held with 2 days remaining, but split-PKGBUILD siblings (`libpipewire`, `pipewire-audio`, `pipewire-alsa`, `pipewire-jack`, `gst-plugin-pipewire`, `alsa-card-profiles`) defaulted to Tier 3 and tried to upgrade. pacman aborted:
>
> ```
> :: installing libpipewire (1:1.6.5-2) breaks dependency 'libpipewire=1:1.6.5-1' required by pipewire
> :: installing libpipewire (1:1.6.5-2) breaks dependency 'libpipewire=1:1.6.5-1' required by pipewire-pulse
> ```
>
> v1.0.4 fixes this by reading the `%BASE%` field from pacman's sync DBs and auto-coupling packages that share a `pkgbase` — they bucket to the highest tier present in the group. Adds `lib32-<X>` auto-coupling for the multilib lockstep case (e.g., `mesa` ↔ `lib32-mesa`, different pkgbases but enforced version-pinned by Arch). Extends `nog unlock --promote` to Tier 2 packages so users can break a lockstep deadlock manually if needed.

### 17a. Layer A — pkgbase sibling coupling

Non-destructive (`nog search` is read-only against pacman repos).

- [ ] **17.1** `$ nog search '^libpipewire$'` — annotation is yellow `[Tier 2 — 15d hold]` (was green Tier 3 in v1.0.3); shares `pkgbase = pipewire` with the explicitly tier-pinned `pipewire`
- [ ] **17.2** `$ nog search '^pipewire-audio$'` — yellow `[Tier 2 — 15d hold]`
- [ ] **17.3** `$ nog search '^gst-plugin-pipewire$'` — yellow `[Tier 2 — 15d hold]`
- [ ] **17.4** `$ nog _debug-hold libpipewire` — `tier: Tier 2`, `window: 15 days`
- [ ] **17.5** `$ nog search '^htop$'` — green `[Tier 3 — 7d hold]`. Coupling only kicks in when a sibling is tier-pinned; htop has no Tier-1/2 sibling in its pkgbase

### 17b. Layer B — `lib32-` auto-coupling

Non-destructive.

- [ ] **17.6** `$ nog search '^lib32-mesa$'` — annotation is red `[Tier 1 — 30d hold]` (Layer B: `lib32-mesa` strips to `mesa` which is Tier 1). Was green Tier 3 in v1.0.3.
- [ ] **17.7** `$ nog search '^lib32-firefox$'` (hypothetical, doesn't exist as a real package — covered by unit test `lib32_inherits_tier2_when_base_is_tier2`)
- [ ] **17.8** `$ nog search '^lib32-libpipewire$'` — yellow `[Tier 2 — 15d hold]` (composed Layer A + B: `lib32-libpipewire` shares `pkgbase = lib32-pipewire` with `lib32-pipewire`, which itself classifies Tier 2 via the `lib32-` rule stripping to `pipewire`)
- [ ] **17.9** `$ nog search '^lib32-htop$'` (hypothetical) — green Tier 3 (strip to `htop` which is Tier 3 — no coupling). Covered by `lib32_of_tier3_stays_tier3` unit test.

### 17c. Live regression — pipewire family upgrades together

The 2026-05-25 failure reproduction. Run this **on a system with a pending pipewire family upgrade** where the base `pipewire` is Tier 2 held but the rest of the family is Ready in v1.0.3.

- [ ] **17.10** `$ nog update` — under v1.0.4, the entire pipewire family (10+ packages sharing `pkgbase = pipewire` and `pkgbase = lib32-pipewire`) appears in the **Held** bucket together, not split across Held + Ready. `nog update` either runs the upgrade in lockstep when the hold expires, or holds the whole group together.
- [ ] **17.11** No `:: installing libpipewire ... breaks dependency 'libpipewire=...required by pipewire` error from pacman — the partial-upgrade dep cascade is structurally prevented.

### 17d. Layer D — `nog unlock --promote` works for any tier

- [ ] **17.12** `$ nog unlock pipewire` (no `--promote`) — informational. Output: `nog: 'pipewire' is Tier 2.` followed by the 15-day hold note and the `--promote` hint. (Pre-v1.0.4 this responded: "no unlock needed (only Tier 1 is ever held by policy).")
- [ ] **17.13** `$ nog unlock pipewire --promote` — force-upgrades pipewire bypassing the Tier 2 hold. Hands off to the configured AUR helper or pacman. (Destructive — runs an actual transaction. Skip if you don't want to upgrade pipewire right now.)
- [ ] **17.14** `$ nog unlock htop --promote` — same behavior for Tier 3 (force-upgrade regardless). Useful when a Tier 3 hold window is blocking a specific package the user wants now.
- [ ] **17.15** `$ nog unlock linux --promote` — Tier 1 behavior unchanged from v1.0.3 (force-upgrade past the 30-day hold).

### 17e. No false positives on coherent systems

- [ ] **17.16** `$ nog update` on a system with no pending Tier-1/2 packages whose pkgbase siblings are pending Tier 3 — bucket distribution should match v1.0.3 behavior exactly for the affected packages. No spurious additional packages move to Held.

---

## Appendix — state restoration after testing

If any test modified state you want reverted:

- Tier pins: `$ nog pin <pkg> --tier=3` returns a package to default
- `manual_signoff`: edit `/etc/nog/tier-pins.toml` back to `false`
- `[aur] helper`: edit `/etc/nog/nog.conf` back to `"auto"`
- Test installs: `$ nog remove <pkg>`
- Held updates: just wait for the hold to expire, or re-run `nog update`

## Appendix — running subsets against the v0.10.0 dev build

Before v1.0 ships, the following test IDs can be run against `cargo build --release`'s binary (`./target/release/nog`) to catch regressions early:

- All of section 1 (adjust version expectation)
- Sections 2, 5, 6, 8, 9 (with the dev config fallback in place)
- Section 11 (root-guard tests)
- Section 12 (config edge cases)
- **Section 16a** (auto-coupling assertions via `nog search` — non-destructive, dev-build-safe)

Install-path tests (section 3) and most of section 7/10/13/14 are best saved for the real post-install dogfood since they mutate system state. Section 16c–16d need an induced desync (VM or test box) since reproducing the v1.0.2 bug requires holding a kernel while letting its headers move.
