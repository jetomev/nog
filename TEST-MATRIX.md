# nog — v1.0 release test matrix

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

- [ ] **1.1** `$ nog --version` prints `nog 1.0.0`
- [ ] **1.2** `$ nog --help` lists all subcommands (`install`, `remove`, `update`, `search`, `pin`, `unlock`) and neither of the hidden `_debug-*` commands
- [ ] **1.3** `$ man nog` opens cleanly; header reads `nog v1.0.0`; **PRIVILEGES AND SUDO** section present
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

Install-path tests (section 3) and most of section 7/10/13/14 are best saved for the real post-install dogfood since they mutate system state.
