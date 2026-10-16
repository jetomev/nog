# Test Matrix — nog v2.0.0 (the crown release: C4 + C5 + C7 of #7; #5)

*Built the night of 2026-10-14 → 15 while Javier slept ("work on them tonight … I want all issues done, nothing open"). §1–§3 are Claude's and done; §4 is Javier's, step by step, on this desktop, with rc1 from `dist-rc/`. The roll-up is `tally-matrix.py`'s, never a hand count.*

**What this version adds** — the command surface across every source: `search` (repositories, AUR, Flatpak, Snap), `list` (Flatpaks and snaps too), `info`, `reinstall`, `tier`, `lock`/`unlock`, `status`, `history`, all with `--json`; `nog clean` across every source (cache, orphans, build trees, unused runtimes, old snap revisions); the linchpin rule for glibc/gcc-libs (#5). Versions: 2.0.0rc1 = this build.

## §1 · Baseline sanity

| # | Check | Expected | Result | Notes |
|---|---|---|---|---|
| 1.1 | `cargo test --release --locked` | green | **PASS** | 284 passed, 0 failed, 5 ignored (259 at v1.9.0) |
| 1.2 | Warning delta vs v1.9.0 | unchanged | **PASS** | 6 → 6 (`generated 6 warnings`) |
| 1.3 | `grep -rn "TODO\|FIXME\|XXX" src/` | empty | **PASS** | |
| 1.4 | `strings target/release/nog \| grep -i CARGO_MANIFEST_DIR` | empty | **PASS** | |
| 1.5 | `man --warnings -l nog.1` | no warnings | **PASS** | |
| 1.6 | Version sync | 2.0.0 in Cargo.toml, Cargo.lock, nog.conf, nog.1, README | **PASS** | |
| 1.7 | rc1 package | built from the AUR recipe, tests in the build | **PASS** | `dist-rc/nog-2.0.0rc1-1-x86_64.pkg.tar.zst`, 284 tests in `check()` |

## §2 · Claude, this desktop — dry runs (nothing changes; `logs/dry-run-200-latest.log`)

41 cases, every one stopping before a change: a read-only command, a name found nowhere, nobody at the keyboard, "n" to the question, or sudo's window refused (`SUDO_ASKPASS=/bin/false`). nog's own records go to a temporary home; history and status read the real logs.

| # | Check | Expected | Result | Notes |
|---|---|---|---|---|
| 2.1 | `nog search calc` · `--json` | four groups (repositories 61, AUR 573, Flatpak 104, Snap 100), tiers, `flatpak/<id>` and `snap/<name>` said; JSON with `install_as` and `sources` | **PASS** | S1, S1j; 30 per source on screen, the rest counted |
| 2.2 | `nog search zzz-nope-nothing` | "Nothing answers to …", the four sources named | **PASS** | S2 |
| 2.3 | `nog list` · `--json` | Flatpak apps and snaps among the rows (`source` flatpak/snap, `remove_as`) | **PASS** | L1, L1j: 1582 rows, 2 flatpaks, 4 snaps (3 bases marked) |
| 2.4 | `nog info cowsay` · `glibc` · `flatseal` · `hello` · `zzz-nope` · `--json` | a card: source, tier and why, the hold in words, the build's age, the source's fields (long lists cut "… and N more"); what is installed wins (flatseal → the installed Flatpak, hello → the installed snap); unknown → "Nothing is known", exit 1 | **PASS** | I1–I5, I1j |
| 2.5 | `nog tier glibc` · `lib32-glibc` · `cowsay` · `linux-zen-headers --json` | the tier *and why*: pinned / the 32-bit half of glibc / the default / the headers of linux-zen | **PASS** | T1–T3, T1j |
| 2.6 | `nog tier bash --set 2` (sudo refused) · `--set 9` · `--set 3` | "Moving bash to Tier 2" then "Not changed" (sudo tee refused) · "9 is not a tier" · "already Tier 3 … Nothing was changed" | **PASS** | T4–T6 |
| 2.7 | `nog lock` · `--json` · `lock zzz-nope` · `lock bash` (sudo refused) · `unlock bash` | "No packages are locked" · `{"locked":[]}` · "not an installed package", exit 1 · "Locking bash … Not locked" (sudo refused) · "Nothing to unlock … not locked", the tier and the promote hint | **PASS** | K1–K4 |
| 2.8 | `nog reinstall zzz-nope` · `hello` (a snap) · `bash` (no terminal) · `bash` + y (sudo refused) · `--yes bash` (sudo refused) | not installed → stops · snapd has no reinstall, the remove-and-install way said, exit 1 · "Nothing is at the keyboard" · "Reinstalling bash" then pacman stopped | **PASS** | R1–R5 |
| 2.9 | `nog history --days 2` · `--json` | the runs newest first, the update decisions, the reboot advice; JSON `runs`/`updates`/`reboots` | **PASS** | H1, H1j (33 runs, 505 update rows, 6 reboots) |
| 2.10 | `nog status` · `--json` | "This computer" (1576 packages, 269 chosen, 33 foreign, 2 Flatpaks, 1 snap), the sources, "Updates: some are ready" (9 ready, 159 held, next release named), "Recently" (last run, last update, last reboot advice) | **PASS** | H2, H2j; ~10 s (the plan) |
| 2.11 | `nog clean --json` | the report: cache 588 items 4.9 GB, orphans 27, aur 0 (temp home), flatpak 7 (3.4 GB), snap 3; `total_bytes` | **PASS** | C1j |
| 2.12 | `nog clean` (no terminal) | the report, then "Not cleaning: Nothing is at the keyboard to pick", exit 1 | **PASS** | C1 |
| 2.13 | `nog clean` + `n` · + Enter · + `9`,`x`,Enter | "Nothing removed" · same · asked again twice ("a number from 1 to 5, several of them, all, or n"), then nothing removed | **PASS** | C2–C4 |
| 2.14 | `nog clean --only typo` · `--only snap` (no terminal) · `--only snap,orphans` + n · `--only aur` | "typo … is not a category here", the categories named · "Nothing is at the keyboard to confirm", exit 1 · "Clean up orphans, snap? [y/N]" → n → nothing removed · "Nothing to clean in aur" | **PASS** | C5–C8 |

## §3 · Claude, the KognogOS VM — real changes (`scripts/vm-run-200.sh`, then `vm-run-200b.sh`)

From snapshot `nog-190-tested` (flatpak and snapd set up, yay present, the state the 1.9.0 run left), the fresh binary in place, every nog run as the VM's user `javier` through `vm-drive` (answers typed when the prompts appear). The first run's driver typed a literal `\n` and matched sudo's password rule on nog's own words ("no password"), so V3a and V4c were rerun with the driver fixed (`vm-run-200b.sh`); nog did the right thing both times (pacman stopped on a bad answer; nog asked again three times, then removed nothing).

| # | Check | Expected | Result | Notes |
|---|---|---|---|---|
| 3.1 | `status`, `history --days 7`, `lock`, `tier glibc`, `info bash`, `search hello --json`, `list --json` as javier | exit 0 each | **PASS** | V0 (`logs/vm-run-200-latest.log`) |
| 3.2 | `nog lock 7zip` (a ready update) → `cat /etc/nog/locks.toml` → `nog lock` → `nog update --json` → `nog unlock 7zip` → the file | "Locking 7zip … stays at 26.03-1"; the file with `"7zip"`; "Locked: 1"; the plan: 7zip held, `locked: true`, note "locked by you — `nog unlock` releases it" (ready 101, held 132); "Unlocking 7zip"; the file empty | **PASS** | V1a–V1d (`logs/vm-200-V1*.log`) |
| 3.3 | `nog lock zzz-nope` | "not an installed package", exit 1 | **PASS** | V1e |
| 3.4 | `nog tier cowsay --set 2` → `nog tier cowsay` → `--set 3` → `nog tier cowsay` | "Moving cowsay to Tier 2"; "cowsay is Tier 2 — Pinned to Tier 2 in tier-pins.toml"; line 176 of the file; back to "Tier 3 — Not pinned: the default" | **PASS** | V2a–V2e |
| 3.5 | `nog reinstall cowsay`, y | pacman "reinstalling cowsay", Done, exit 0 | **PASS** | V3a (second run) |
| 3.6 | `nog install --yes flatpak/com.tominlab.wonderpen` → `nog reinstall --yes wonderpen` | installed; "Handing off to flatpak with your yes", `flatpak install --reinstall`, Done | **PASS** | V3b, V3c |
| 3.7 | `nog install --yes snap/hello-world` → `nog reinstall hello-world` → `nog reinstall zzz-nope` | installed; "snapd has no reinstall step … nog remove hello-world && nog install snap/hello-world", exit 1; "not installed", exit 1 | **PASS** | V3d–V3f |
| 3.8 | Garbage made: `pacman -S --asdeps sl` (5 orphans), `nog remove --yes wonderpen` (4 unused runtimes), hello-world rev 27 then refresh (an old revision), yay's `snapd` tree → `nog clean --json` | orphans 5 (327 MB), aur 1 (36 MB), flatpak 4 (1.7 GB), snap 1 | **PASS** | V4a, V4b |
| 3.9 | `nog clean` → `all`, pacman y, flatpak y | pacman -Rns the 5 orphans; the yay tree gone (`completion.cache` and `vcs.json` stay); `flatpak uninstall --unused` lists 5 refs and removes them; `snap remove --revision=27 hello-world`; "Cleaned · About 1.9 GB". After: orphans 0, disabled revisions 0, no runtimes, no trees; `clean --json` after: cache 5 (the removed orphans' files), the rest 0 | **PASS** | V4c, V4d (second run) |
| 3.10 | an old revision again → `nog clean --only snap` + n → `--only snap --yes` | "Clean up snap? [y/N]" → n → "Nothing removed", the revision stays; `--yes` → removed, 0 left | **PASS** | V4e, V4f |
| 3.11 | `nog info cowsay` · `info hello-world` (the installed snap) · `search hello --json` · `list --json` · `history --days 1` | the card (Explicitly installed, 683-day-old build); "From Snap … Revision 29 · Tracking latest/stable"; repo 5 / aur 23 / flatpak 1 / snap 100; 3 snaps in the list; the day's runs newest first | **PASS** | V5a–V5e |

## §4 · Javier, this desktop — step by step

*rc1 is rc of 2.0.0: `dist-rc/nog-2.0.0rc1-1-x86_64.pkg.tar.zst`. Each step: Claude says what it does and what to expect, Javier types one line and pastes the screen or says PASS. Findings → F-n issues, fixed into the same unpublished 2.0.0 (rc2, …).*

| # | Check | Expected | Result | Notes |
|---|---|---|---|---|
| 4.1 | `nog install ~/Programs/nog/dist-rc/nog-2.0.0rc1-1-x86_64.pkg.tar.zst` | the package-file install; `nog --version` says **2.0.0** | PASS | |
| 4.2 | `nog --help` | the new commands listed: info, reinstall, tier, lock, status, history; clean's new words | PASS | |
| 4.3 | `nog status` | "This computer", "Updates: …", "Recently" — the dashboard in plain words; ~10 s | PASS | |
| 4.4 | `nog history --days 2` | yesterday's bench runs, newest first; the update decisions; the reboot advice | PASS | |
| 4.5 | `nog search calculator` | four groups with tiers; the Flatpak rows say `flatpak/<id>`; the snap rows the publisher | PASS | |
| 4.6 | `nog info flatseal` | "From Flatpak · installed", Tier 3 and why, the hold in words, flatpak's fields (Ref, Origin, Runtime, Commit) | PASS | |
| 4.7 | `nog info glibc` | "From the core repository · installed", Tier 1 — Pinned, 30 days, Required By cut to 12 names "… and N more" | PASS | |
| 4.8 | `nog tier lib32-glibc` | "lib32-glibc is Tier 1 — The 32-bit half of glibc, which is Tier 1" | PASS | |
| 4.9 | `nog lock cowsay` (install cowsay first: `nog install cowsay`) | sudo's window; "Locking cowsay … stays at 3.8.4-1"; then `nog lock` lists it; `nog update` shows it Held, "locked by you" (answer n to the handoff) | PASS | lock + list as expected; `nog update` showed nothing for cowsay because cowsay has no pending update (3.8.4-1 is current) — the matrix expectation was wrong, not nog; see F-17 |
| 4.10 | `nog unlock cowsay` | sudo's window; "Unlocking cowsay"; `nog lock` → "No packages are locked" | PASS | |
| 4.11 | `nog tier cowsay --set 2` → `nog tier cowsay` → `--set 3` | moved to Tier 2 (sudo's window), said so, moved back | PASS | Claude handed all three commands in one step; only `--set 2` ran at first (cowsay stayed Tier 2); `--set 3` run on its own afterwards, verified Tier 3 and the pin gone |
| 4.12 | `nog reinstall cowsay` | pacman's "Proceed?" → y → "reinstalling cowsay", Done | PASS | |
| 4.13 | `nog reinstall hello` (the snap on this desktop) | the refusal with the way out; nothing changed | PASS | |
| 4.14 | `nog clean` → look, then `n` | the report: cache ~4.6 GB, 27 orphans (~900 MB), yay's trees (~1.3 GB), 7 unused runtimes (~3.4 GB), 3 old snap revisions; nothing removed | PASS | cache 4.6 GB, 27 orphans 918 MB, 22 yay trees 1.2 GB, 7 runtimes 3.4 GB, 3 snap revisions 114 MB; total 10.3 GB |
| 4.15 | `nog clean` → pick the categories you want (e.g. `1 3 5`: cache, yay's trees, snap revisions) | each removed by its tool: sudo's window for the cache files and snap; "Cleaned" | PASS | Javier picked 1 3 4 5; cache, 22 yay trees, 9 Flatpak refs, 3 snap revisions removed; "About 9.4 GB". flatpak 1.18.4 did NOT ask (nog said it would) — F-18 |
| 4.16 | `nog clean --only orphans` → look at pacman's list → y or n | pacman -Rns asks with the full list; your answer decides | PASS | Javier answered n at pacman; all 27 kept. nog wrongly called it a failure (status 1) — F-19 |
| 4.17 | `nog remove cowsay` | as before (1.9.0): nothing in this release changes remove | PASS | sanity |
| 4.18 | `nog update` | as before; a locked package would show "locked by you" (none now); answer n | PASS | sanity — run as part of 4.9 (10 ready, 239 held, n at the handoff, Cancelled) |
| 4.19 | `nogforge` → Search cowsay → Install → Uninstall | nogForge 1.4.0 on nog 2.0.0: unchanged (its Flatpak/snap rows and Maintenance page come with nogForge 1.5.0) | PASS | sanity — nogForge 1.4.0-2 on nog 2.0.0rc1 |
| 4.20 | `nog install ~/Programs/nog/dist-rc/nog-2.0.0rc2-1-x86_64.pkg.tar.zst` | rc2 (F-17, F-18, F-19 fixed); `nog --version` 2.0.0 | PASS | rc2 |
| 4.21 | `nog lock jq` → `nog update` (n) → `nog unlock jq` | jq is up to date, so no table shows it; below the tables: "nog: locked by you, nothing pending — jq (up to date; `nog unlock` releases it)." | PASS | F-17 proven: the line showed below the fence note; unlocked after |
| 4.22 | `nog clean --only orphans` → y → **n** at pacman | pacman's list, n; then "nog: orphans kept — you answered n at pacman's question." and "Nothing removed." — green Done, no "Stopped", no "with problems". The legend's flatpak line says flatpak removes its list, the pick is the yes | PASS | F-18 (legend) and F-19 (n at pacman → "orphans kept", "Nothing removed", green Done) proven on 10-16 |

## Findings

| # | Step | What happened | Severity | Decision |
|---|---|---|---|---|
| F-17 | 4.9 | A locked package with no pending update is invisible in `nog update`: cowsay was locked, up to date, and the update screen said nothing about it. Correct (there is nothing to hold) but a lock should stay visible — a line like "Locked by you: cowsay (nothing pending)" in the summary. | low | fixed in rc2: `nog update` prints "locked by you, nothing pending — <names>" below the tables (`locks::idle`, +1 test); re-test 4.21 |
| F-18 | 4.15 | nog says "flatpak shows its list and asks first", but flatpak 1.18.4 does not ask for `uninstall --unused`: it printed 9 refs and removed them straight away (terminal attached, no `-y`). The outcome is right (picking 4 at nog's question is the consent) but the promise is false. Fix: the legend, the notice and nog.1 say flatpak works out the list and removes it, your pick at nog's question being the yes. | low | fixed in rc2: legend, notice and nog.1 say flatpak removes its list and the pick is the yes; re-test 4.22 (legend) |
| F-19 | 4.16 | Answering **n** at pacman's "Do you want to remove these packages?" made nog say "orphans did not finish cleanly", "Cleaned, with problems", and exit with status 1. The 27 orphans were all still installed, so the person's no worked; nog mistook a choice for a failure (pacman exits 1 for both). Fix: re-count orphans after pacman — all still there = "kept: you answered n" (status 0), none = cleaned, some = a real problem. | medium | fixed in rc2: orphans re-counted after pacman; all left = "orphans kept — you answered n", "Nothing removed", status 0; re-test 4.22 |
