# nog — v1.0.9 release test matrix

Focused matrix for the **"Ironhold" security cycle** (foreign fence, source kill switches, held-table sort). Baseline sanity is repeated with v1.0.9 strings; the full end-to-end surface is covered by the rolling v1.0.5 matrix plus the per-release sections 16–18. This file adds **section 19**.

## How to run

1. Work top to bottom; each section assumes the previous passed
2. Tick the checkbox (`[x]`) next to each test as you verify it
3. If a test fails, stop and file a bug — don't continue past a broken section

Conventions:
- `$` — run as your regular user
- **EXPECT:** — the observable outcome that makes the test pass

---

## 1. Baseline sanity (v1.0.9 strings)

- [ ] **1.1** `$ nog --version` prints `nog 1.0.9`
- [ ] **1.2** `$ nog --help` lists all subcommands — now including `activate` and `deactivate` — and neither hidden `_debug-*` command
- [ ] **1.3** `$ man nog` opens cleanly; header reads `nog v1.0.9`; **COMMANDS** covers `activate`/`deactivate`; **FILES** covers `/etc/nog/sources.toml` and the pacman.conf backup convention
- [ ] **1.4** `cargo test --release --locked` → **54 passed, 0 failed**

## 19a. The foreign fence (#2)

- [ ] **19a.1** `$ nog update` with a helper configured and pending updates: after the tables (and any Unknown prompts), a subtext block prints `foreign fence — N AUR/local package(s) shielded from the handoff`, where N = `pacman -Qmq | wc -l` minus any AUR packages nog cleared this run
- [ ] **19a.2** Proceeding: the helper output shows `--ignore` honored; **no foreign package upgrades unless it appeared in Ready** (or was a user-approved Unknown)
- [ ] **19a.3** AUR-query-failure path: when `<helper> -Qua` errors, the warning now ends with `the foreign fence will shield ALL foreign packages from this run's handoff`
- [ ] **19a.4** Regression guard: unit test `fence_replays_the_august_first_bypass` present and green (the 2026-08-01 live bypass, reproduced)

## 19b. AUR kill switch (#3)

- [ ] **19b.1** `$ nog deactivate aur` → sudo prompt (tee), confirmation block explains: update won't query AUR, handoff runs through pacman, `nog install` routes pacman-only; `/etc/nog/sources.toml` exists with `aur = false`
- [ ] **19b.2** `$ nog update` while deactivated → first line: `AUR is DEACTIVATED (kill switch) — official repos only`; **no** "N AUR update(s)" line; handoff (if reached) says `Handing off to pacman`
- [ ] **19b.3** `$ nog activate aur` → confirmation names the helper setting from nog.conf (untouched, e.g. `'auto'`); next `nog update` queries the AUR again
- [ ] **19b.4** Idempotence: repeating either command prints `already active/deactivated` and writes nothing
- [ ] **19b.5** Fail-closed: corrupt `/etc/nog/sources.toml` by hand → any nog command warns loudly and treats **every source as DEACTIVATED**; `nog activate aur` rewrites the file cleanly

## 19c. chaotic-aur kill switch (#4)

- [ ] **19c.1** `$ nog deactivate chaotic-aur` → backup path printed (`/etc/pacman.conf.nog-bak-<stamp>`, exists, `--preserve=all`); `[chaotic-aur]` section lines carry the `#nog# ` prefix; DB refresh lists **no** chaotic-aur
- [ ] **19c.2** While deactivated: `pacman -Sl chaotic-aur` errors (repo unknown); installed chaotic packages remain installed
- [ ] **19c.3** `$ nog activate chaotic-aur` → section restored; refresh syncs chaotic-aur again
- [ ] **19c.4** **Byte-exact restore:** `diff <first backup> /etc/pacman.conf` → identical
- [ ] **19c.5** User comments inside the section survive a full round trip (also unit-tested)
- [ ] **19c.6** `$ nog deactivate ghost-source` → error listing valid sources, exit non-zero

## 19d. Held table sort (#6)

- [ ] **19d.1** `$ nog update` Held table: days-remaining ascending; within one value, alphabetical; stable across two consecutive runs
- [ ] **19d.2** The day's CSV run log rows appear in the same order as the printed table
