# Test Matrix — nog v2.1.0 (unattended updates: #56, F-21 #57, F-22 #58, F-23 #59, F-24 #60)

*Built the night of 2026-10-22 → 23 while Javier slept (stage 02 of the KognogOS road: "Everything you can do without me"). §1–§3 are Claude's and done; §4 is Javier's, step by step, on this desktop, with rc1 from `dist-rc/`. The roll-up is `tally-matrix.py`'s, never a hand count.*

**What this version adds:** `nog update --yes`, so an installer, a timer or a script can update with nobody at the keyboard (Ready packages go in; Unknown ones are kept back and listed; the AUR's build review waits for a person). Long AUR builds keep sudo awake (`--sudoloop`), and a password nobody typed in time is named as the cause. An update that leaves a step unfinished exits **3** and its runs line says **incomplete**. With `--yes` and nobody at the keyboard, nog checks first whether sudo would need a password. Error messages say the exit status once. Versions: 2.1.0rc1 = this build.

**Where these came from:** Javier's real `nog update` on 2026-10-22 (`~/.local/share/nog/logs/20261022-170019 update.log`): 184 installed, then snapd's seven-minute AUR build ended at "sudo: timed out reading password", and the run still said "done" with exit 0.

## §1 · Baseline sanity

| # | Check | Expected | Result | Notes |
|---|---|---|---|---|
| 1.1 | `cargo test --release --locked` | green | **PASS** | 294 passed, 0 failed, 5 ignored (285 at v2.0.0); each new test seen failing first |
| 1.2 | Warning delta vs v2.0.0 | unchanged | **PASS** | 6 → 6 (`generated 6 warnings`) |
| 1.3 | `grep -rn "TODO\|FIXME" src/` | empty | **PASS** | |
| 1.4 | `strings target/release/nog \| grep CARGO_MANIFEST_DIR` | empty | **PASS** | |
| 1.5 | `man --warnings -l nog.1` | no warnings | **PASS** | the new EXIT STATUS section included |
| 1.6 | Version sync | 2.1.0 in Cargo.toml, Cargo.lock, nog.conf, nog.1, README | **PASS** | |
| 1.7 | rc1 package | built from the AUR recipe, tests in the build | **PASS** | `dist-rc/nog-2.1.0rc1-1-x86_64.pkg.tar.zst`, 294 tests in `check()`; the binary inside says 2.1.0; no `.sig` beside it. fakeroot printed "payload not recognized!" once, as in 2.0.0rc1 (that package passed 54/54) |

## §2 · Claude, this desktop — dry runs (nothing changes)

Every case stops before a change: nobody at the keyboard, or sudo refused at once by a stand-in `sudo` first on the script's PATH. nog's own records go to a temporary home.

| # | Check | Expected | Result | Notes |
|---|---|---|---|---|
| 2.1 | `nog update </dev/null` | "Not updating", the cause said (nobody at the keyboard), `--yes` offered; exit 1 | **PASS** | D1, `logs/dry-run-210-latest.log` |
| 2.2 | `nog update --yes </dev/null`, sudo would need a password | "Not updating … sudo still asks for a password, and nobody is at the keyboard", how to set it up; exit 1 | **PASS** | D2 |
| 2.3 | `nog update --yes` at a terminal, sudo refused | "Begin the handoff? yes (--yes)"; pacman's step fails → the run is cancelled, exit 1, runs line `stopped` | **PASS** | D3. Claude first expected 3; wrong: nothing was updated, so the run stopped. 3 means a *later* step failed after pacman's went through (§3, 3.4) |
| 2.4 | `nog update --help` · `man -l nog.1` | `--yes` described; EXIT STATUS lists 0, 1, 2, 3 | **PASS** | D4 |
| 2.5 | `scripts/dry-run-200.sh` · `dry-run-190.sh` with the 2.1.0 binary | OK, no crash; exits as at 2.0.0 | **PASS** | 41 + 26 cases. Two changed result because this desktop changed, not nog: T6 (`tier bash --set 3`) — bash is now pinned Tier 1 in `/etc/nog/tier-pins.toml` (changed 10-22), so a move is attempted and sudo refused; C6 (`clean --only snap`) — snapd is installed now |
| 2.6 | T6's refusal message | "sudo tee /etc/nog/tier-pins.toml exited with status 1" | **PASS** | it read "exited with status exit status: 1" → **F-24**, fixed and re-run |

## §3 · Claude, the KognogOS VM — real changes (`scripts/vm-run-210.sh`)

From snapshot `nog-210-before`, the fresh binary in place, the package lists synced (147 pending). Every nog run as the VM's user `javier`, through the guest agent with stdin from `/dev/null`: no terminal at all, so no password can ever be typed. The VM's leftover 1.9.0 rule (`90-test-nopasswd`, anything without a password) is moved aside for Y2–Y4. The VM went back to `nog-210-before` afterwards. Log: `logs/vm-run-210-20261023-001116.log`. (The first run, 00:04, kept that rule, so its Y2 updated 41 packages: right for nog, a wrong premise of Claude's.)

| # | Check | Expected | Result | Notes |
|---|---|---|---|---|
| 3.1 | Y1: `nog update` | "Not updating", the cause said; exit 1; pending 147 → 147 | **PASS** | |
| 3.2 | Y2: `nog update --yes`, sudo would ask (`sudo -n pacman` → 1) | "Not updating", the password said; exit 1; pending 147 → 147 | **PASS** | |
| 3.3 | Y3: `nog update --yes`, a sudo rule for pacman only | "Begin the handoff? yes (--yes)"; pacman with `--noconfirm`; "kept back from the AUR: proton-ge-custom-bin, snapd"; "Update finished!"; exit 0, runs line `done` | **PASS** | 41 updated, pending 147 → 106 (the rest held) |
| 3.4 | Y4: `nog update --yes`, a stand-in flatpak (one dated app whose update fails, one app without a date) | "--yes keeps 1 Unknown package(s) back: org.nogbench.NoDate"; "the flatpak step exited with status 1"; "Continue with the remaining sources? no (--yes takes the default)"; "Update finished, with 1 step(s) that did not complete"; exit **3**, runs line **incomplete** | **PASS** | the update log: 1 unknown `skipped`, 1 ready `did not complete (status 1)` |

The VM ran the binary from before F-24 (message text only; covered by its test and 2.6).

## §3b · Claude, the KognogOS VM — rc2 (`scripts/vm-run-210b.sh`)

From `nog-210-before`, a real `nog update --keep proton-ge-custom-bin` at a keyboard (`vm-drive` types the password when sudo asks); the VM's no-password rule moved aside; snapd 2.78 and nogforge 1.5.0 built from the AUR; the VM restored after. Three runs were needed, each failure the bench's own: run 1, sudo refused the password the older scripts carried (it never matched the VM; every earlier bench had the no-password rule on), so the bench now sets a throwaway one per run; run 2, the control hung 56 minutes at yay's cleanBuild menu (the driver's keystroke was lost), so yay's menus are now off in the VM's yay settings — and run 2 is where **F-27** showed (below). Logs: `logs/vm-run-210b-*.log`, `logs/vm-210b-S*.log`.

| # | Check | Expected | Result | Notes |
|---|---|---|---|---|
| 3b.1 | S0: rc2's plan in the VM (read-only) | perl already 5.44 there, apparmor's rebuild held: F-25 has nothing to show in this VM | N/A | perl moved before this snapshot; F-25 rests on its tests (the real 10-23 data) and the live check of this desktop's file lists |
| 3b.2 | run 2 · S2: rc2 (`--sudoloop`), sudo forgetting after 1 minute, yay's menus on | the AUR step runs | FAIL | **F-27**: yay's keep-awake `sudo -v` asked for the password in the middle of yay's cleanBuild menu; answers crossed, sudo timed out, a dozen prompts; fixed (nog asks `sudo -v` itself right before the helper) |
| 3b.3 | run 3 · S1 control (2.0.0) and S2 (rc2 + F-27), sudo forgetting after 1 minute | both update, build snapd and nogforge, exit 0 | PASS | both: 2 prompts (pacman's, and one at the install after the builds). **This does not prove `--sudoloop`**: yay's loop refreshes about every 5 minutes, matched to Arch's 5-minute default, so a 1-minute setting outruns it; and snapd built in 45 s. See 3b.5 |
| 3b.4 | run 3 · S2: the computer-wide record (pkg-140) | `/var/lib/nog/last-update` owned by root, dated the update; no per-user record | PASS | `-rw-r--r-- 1 root root 0 Oct 23 18:45 /var/lib/nog/last-update`; `~/.local/share/nog/last-update` absent |
| 3b.5 | run 4 · S3: rc2, sudo at its normal memory (5 min), a watcher on the processes | yay started with `--sudoloop`; the whole update asks once | PASS | the watcher: `yay --sudoloop -S snapd nogforge --ignore proton-ge-custom-bin,…`; **1 password prompt** for pacman, the make dependencies, both builds and both installs (18:49 → 18:54:56). Not seen: the loop's own `sudo -v` calls (too brief for a 1-second watcher), nor a build longer than 5 minutes (snapd builds in 45 s in the VM) |

## §4 · Javier, this desktop — step by step

*rc1: `dist-rc/nog-2.1.0rc1-1-x86_64.pkg.tar.zst`. Each step: Claude says what it does and what to expect, Javier types one line and pastes the screen or says PASS. Findings → F-n issues, fixed into the same unpublished 2.1.0 (rc2, …).*

| # | Check | Expected | Result | Notes |
|---|---|---|---|---|
| 4.1 | `nog install ~/Programs/nog/dist-rc/nog-2.1.0rc1-1-x86_64.pkg.tar.zst` | the package-file install; `nog --version` says **2.1.0** | PASS | 10-23 14:34; 2.0.0-1 → 2.1.0rc1-1 (the banner said v2.0.0: the installed nog ran the install, as expected) |
| 4.2 | `nog update --help` | a `--yes` line: Ready goes in, Unknown kept back, the AUR's review waits for a person | PASS | 10-23; the `--yes` line as expected |
| 4.3 | `nog update </dev/null` | the tables, then "Not updating — Nothing is at the keyboard …", `--yes` offered; "Stopped"; nothing changed | PASS | 10-23 14:48; 10 ready, 166 held, 0 unknown; "Not updating" with the cause, `--yes` offered; Stopped, status 1 |
| 4.4 | `sudo -K` | nothing printed: sudo forgets the password it remembered from 4.1 (it keeps it ~5 minutes; without this, 4.5 would really update) | PASS | 10-23; nothing printed |
| 4.5 | `nog update --yes </dev/null` | "Not updating — --yes answers nog's and pacman's questions, but sudo still asks for a password …", how a timer would set it up; "Stopped"; nothing changed | PASS | 10-23 14:55; "Not updating": the password said, how a timer sets it up; no password prompt appeared; Stopped, status 1 |
| 4.6 | `nog update` (the real one; your call, y or n) | as before at the terminal. If you say y and the AUR has updates (snapd, proton-ge-custom-bin): the password once when the AUR step starts, then **no second password** however long snapd builds (`--sudoloop`). The end says "Done", or, if a later step fails, "with N step(s) that did not complete" and "Run nog update again to finish them" | PASS | 10-23 15:15; 9 repository packages + snapd 2.78; one password (at pacman's step); Done. **F-21 not stress-tested here**: snapd 2.78 had been built on 10-22 (the run that timed out at the install), so yay skipped the build — no long build happened. perl 5.42 → 5.44 left apparmor's and subversion's perl modules behind (their rebuilds held 1 more day) → **F-25** |
| 4.7 | `nog history --days 1` | today's runs newest first: 4.3 and 4.5 `stopped`; 4.6 `done` (or `incomplete` if a step failed, or `stopped` if you said n) | PASS | 10-23 16:11; 15:15 done, 14:55 and 14:48 stopped. The reboot advice below printed upside down → **F-26** |
| 4.8 | `nogforge` → Update (4) → look, then q | nogForge 1.5.0 on nog 2.1.0: the plan read without errors (after 4.6: 0 ready, the held list with its notes) | PASS | 10-23 ~16:20; "1 to update · 0 kept back · 165 held": subversion's hold had just run out (half of F-25 resolved itself), no error |

## §5 · Javier, this desktop — rc2 (F-25, F-26, F-27, the computer-wide record, the trimmed list)

*rc2: `dist-rc/nog-2.1.0rc2-1-x86_64.pkg.tar.zst`. Only what changed since rc1. F-25 cannot be shown here any more (perl already moved on 4.6); it is proven by its tests with the real data and the live check of this desktop's file lists (`usr/lib/perl5/5.42/` → apparmor, subversion). F-27 shows only when an AUR package is Ready and the pacman step outlasts sudo's 5 minutes; it is proven in the VM (§3b).*

| # | Check | Expected | Result | Notes |
|---|---|---|---|---|
| 5.1 | `nog install ~/Programs/nog/dist-rc/nog-2.1.0rc2-1-x86_64.pkg.tar.zst` | `nog --version` says 2.1.0; pacman says `/etc/nog/tier-pins.toml installed as /etc/nog/tier-pins.toml.pacnew` (this desktop runs KognogOS's list since 10-22, and pacman keeps an edited file: the trimmed default lands beside it, unused) | PASS | 10-23; 2.1.0rc2-1 installed, the .pacnew warning shown, nog 2.1.0 (pacman -Q and the .pacnew checked) |
| 5.2 | `nog history --days 1` | the reboot advice reads top to bottom: "IMPORTANT: It is highly recommended to reboot the system!" first (F-26) | PASS | 10-23 19:32; IMPORTANT first, "mesa was updated this run." last; the update decisions in written order too. Spotted on the way: the dry runs' status/history cases had written into the real runs log; dry-run-200.sh now reads a copy |
| 5.3 | `nog update` → y | subversion (Ready since 4.8) and whatever else is Ready; one password; Done | PASS | 10-23 20:05; subversion 1.14.5-6 in, one password, Done; perl's hook now names 1 package (apparmor, its rebuild ready tomorrow) |
| 5.4 | `ls -l /var/lib/nog/last-update` | a file owned by root, dated 5.3's time: the record for the whole computer (pkg-140) | PASS | checked by Claude on the machine: `-rw-r--r-- root root 0 2026-10-23 20:06:25 /var/lib/nog/last-update`; rc1's per-user record (15:16) stays, older, unused while the newer counts |

## Findings

| # | Step | What happened | Severity | Decision |
|---|---|---|---|---|
| F-27 | 3b.2 | With `--sudoloop`, when sudo had forgotten the password during a long pacman step, yay's background `sudo -v` prompted in the middle of yay's own cleanBuild menu: two programs reading the keyboard at once; answers crossed, sudo timed out, a dozen prompts followed, the step failed. | medium | rc2 (#63): `aur::sudo_refresh` — nog runs `sudo -v` alone right before the helper; refused = "sudo was not given the password, so the build did not start"; tests seen failing first |
| F-26 | 4.7 | `nog history` printed the 10-22 update's six-line reboot advice upside down: each day was reversed row by row, splitting what one run wrote; `nog status` took the last line as the headline. | low | rc2 (#62): `history::newest_first` reverses runs, not rows; test seen failing first |
| F-25 | 4.6 | perl went 5.42 → 5.44 while apparmor 4.1.8-2 and subversion 1.14.5-6, rebuilt for it, were still Held (1 day): perl's hook warned that `/usr/lib/perl5/5.42` holds data from 2 packages the new perl won't use. Arch expresses this only in file paths (no versioned dependency, no ABI provide); python has the same shape, larger. | medium | rc2 (#61): a coupling rule — an interpreter changing series waits for the installed packages under its old module directory |
| F-24 | 2.6 | Seven error messages said "exited with status exit status: 1": nog's words, then Rust's own description of how a program ended. Seen in the 2.0.0 dry run's T6 with the 2.1.0 binary. | low | fixed in rc1 (#60): `machine::status_text` gives "1", or "signal 9"; test `status_text_says_the_number_once` seen failing first; 2.6 re-run |
