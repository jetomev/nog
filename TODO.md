# nog — the list

**Current release: v1.8.0** (4 Oct, GitHub + AUR): the install shows where each package comes from (#47). Before: v1.7.0 (4 Oct, GitHub + AUR) — was: test package built 4 Oct: `NOG_EVENTS` for nogForge's steps view, `repo/name` installs from that source (F-11) — issue #45; waits on Javier's desktop test (nogforge matrix §2), then C3 moves to v1.8.0. **Current release: v1.6.1** (4 Oct 2026) — `nog update a b c` updates only those (#44), a name found nowhere said plainly (#43); GitHub Latest + AUR. Earlier: v1.5.8 (2 Oct 2026) — installs and removals ask (#38), a short install screen with the ready table (#37), designed notices (#39), pacman's question under its table (#40), and a banner + closing (logs, thanks) on every run, with `nog-runs.csv`. 224 tests. Javier tried it in the VM: *"all looks much better"*. Same day: v1.5.7 (#34–#36), and the key now comes from GitHub (#33).
nog is the package updater for KognogOS. It holds new packages back for a waiting period set by their tier, so a bad update has time to be noticed before it reaches this computer.

**Updated after every step.** The full story behind each item is in its GitHub issue.

---


- [x] **Answered (3 Oct): a name plus package files in one `nog install` doing nothing is by design** — the man page (since 1.5.0): "Names and files cannot be mixed in one command; they are separate pacman operations, and splitting one request could leave half of [the request done]". To consider: say so on screen when someone mixes them, instead of nothing visible (it printed nothing the guest agent captured).

- [x] **SHIPPED in v1.6.1 (4 Oct, tag + GitHub + AUR `cd3ae77`, 237 tests, warnings 6; #43 #44 closed). Javier to install from the AUR.** **F-10 · #44 (1.6.1) · `nog update a b c` = only those** (Javier, 4 Oct, nogForge Update test: "a specific list, it's intentional … a held one … not promoted first: don't execute it"). **Code done on main**: one table UPDATING ONLY WHAT YOU NAMED; everything else kept back this time (coupling + handoff unchanged, fence still applied, its note hidden); a held / partner-bound / not-pending / also-kept name stops the run before any question, one line each with what to do, exit 1. 235 tests, warnings 6. Tried on the desktop with the built binary (scratch HOME, nothing installed). rc package built (`dist-rc`, 235 tests in the build) and **installed by Javier 09:59 (only nog changed)**. **10:04: Javier pressed u in nogForge (vde2 + wolfssl): only those two shown and installed, exit 0.** pacman still printed ~70 "ignoring package upgrade" lines → Javier chose (b): nog says first that pacman will list what it skips (rc.2, 236 tests). rc.2 **installed by Javier 10:12**. **10:33: Javier promoted git through nogForge: `update freerdp git --promote git`, only those two, the note said 68 skipped and pacman listed 68, both in, exit 0 ("exactly what was described!").** **Next:** F-9 #43, then the 1.6.1 release; at release: README + man page + CHANGELOG, close #44.
- [x] **F-9 · #43 — shipped in v1.6.1** — a name found nowhere is reported as "pacman stopped: you answered no, or it hit a problem"; say "not found in the repositories or the AUR" and name the tool (Javier, 3 Oct 22:34).
- [x] **AUR 1.6.0 installed by Javier (3 Oct 22:33)** — built from the AUR recipe, 234 tests passed in the build.
- [x] **v1.6.0 RELEASED 3 Oct (Javier: "push … to final versions … very solid")** — tag, GitHub Latest (signed assets), AUR. #41 #42 closed; #30 closed (superseded by #41); #7 advanced. Tests 224 → 234, warnings 6. Javier tests the AUR install + nogForge Update on 4 Oct; issues as needed.
- [x] **rc.5 (3 Oct): a recorded run names only its `.log`** — Javier chose "A": the CSVs stay as nog's bookkeeping (the hold clock reads the update CSV; nogForge's tables, 90 days, runs without a terminal), unnamed. Javier considered dropping the CSVs; why not: see #42.
- [x] **#42 · each run kept whole as a `.log` (3 Oct, rc.4)** — Javier's idea for nogForge's nog Logs. util-linux `script`, output only, 30 days (his number). Tests 231 → 234. **Goes in the 1.6.0 changelog.** Close #42 at the release.
- [x] **F-8 · #41 · `nog install` installs only what was asked (3 Oct, rc.3)** — Javier: "if I ask pacman to install a package, it just installs the package. nog has to do the same, not update the system." The update-before-install path (#30, v1.5.6–1.5.8) removed; old lists are named only when an install fails. Checked with print-only stand-ins. **Goes in the 1.6.0 changelog** (a behaviour change from 1.5.x). Close #41 at the release.
- [x] **`--promote` (3 Oct, rc.2, `acddcb4`)** — held → ready, partners follow. Note: that commit message said "tests 235 → 236"; wrong, the real count is 231 passed + 5 ignored.
- [x] **v1.6.0 for nogForge — built 3 Oct (09:00–09:25), as 1.6.0-rc.1, NOT released, NOT committed (GPG passphrase expired; commit when Javier unlocks it)**: `nog list --json` (protection in words: Tier 1 / base set / needed by), `nog search --json` (repos + AUR via helper), `nog update --json` (same code as a real update, stops before any question; stdout = one JSON document, the rest to stderr; no banner, no run-history line), `--keep a,b` (held before the coupling rules — checked: ldb takes libwbclient + smbclient with it), `NOG_ASKPASS=1` → every sudo gets -A, helper gets `--sudoflags -A`. Tests 229 → 234, warnings 6 → 6. `scripts/make-rc-package.sh` → `dist-rc/nog-1.6.0rc1-1-x86_64.pkg.tar.zst` (package check ran every test). Next: Javier's run (nogForge matrix §6.1), then the release: README + man page for the new options, version 1.6.0 everywhere, CHANGELOG, tag, GitHub, AUR, close/advance #7.

- [ ] **To watch (3 Oct, not a bug yet):** #27's fix works — linux-lts (first seen Jul 28) and mkinitcpio (Aug 12) are past their 30-day window and wait only on the 7-day safety rule (newest builds Oct 1 → ready Oct 8). But if new builds keep arriving less than 7 days apart, the safety wait could slip again (a small cousin of #27). Check after Oct 8 whether both went in at the next update.
## At the next release
- [ ] **The AUR description, at the next release** (Javier, 2026-10-09): the AUR `pkgdesc` (and `.SRCINFO`) gets the same "where it runs" words as the README, GitHub About and kognogos.org — distribution · desktop · plain text console. Not pushed on its own: AUR pushes stay one per proven version.

## Done — v1.5.0 released (29 Sep 2026)
v1.5.0 on GitHub (Latest, signed) and the AUR, installed on this desktop through its own `nog install <file>`. Tags v1.4.2 and v1.4.3 ride along. Issues #14, #16, #17, #19–#22 closed. 172 tests, 6 warnings.

## Still open from this release
- [x] **#24 (F-1)** — fixed and shipped as v1.5.1; installed on this desktop with `nog install <file>` (17:3x)
- [x] Fresh install from the AUR through yay — works (Javier, 17:15)
- [ ] Watch the first real update that installs something: pacman's own "Proceed?" visible, `installed` rows in the log (matrix 7.3, 7.4)
- [ ] Not seen yet on real data: the library scan firing (2.7), the reboot log (4.10), the #14 prompt line (3.1)
- [x] AUR search index caught up to 1.5.0-1 (checked 29 Sep, ~17:30)
- [x] README AUR badge shows 1.5.1 on github.com (checked through GitHub's own image cache, ~18:10)

## Done · v1.8.0 — the install shows where each package comes from (C3 part 1) · released 2026-10-04 (#47)
- [x] Javier's option A (4 Oct): table (package, version, source, tier) before anything is downloaded; "Install these? [Y/n]" only when the AUR is involved; "Handing off to pacman/yay: …"; not-found stops first. 242 tests
- [x] rc.1 built (`dist-rc/nog-1.8.0rc1-1-x86_64.pkg.tar.zst`, tests in a terminal)
- [x] Javier, rc.1 (15:36–15:37): installed only nog; `install sl` → table (extra) + pacman handoff, installed; `install neofetch` → table said "chaotic-aur — unifetch provides it", unifetch went in; `install zzzz` → stopped before anything, not found
- [x] Javier, 18:12: `nog install aur/neofetch` → table (AUR — built by yay), "Install these? [Y/n]", the yay handoff line, yay's menus and build, pacman's conflict question (unifetch removed): neofetch 7.1.0-2 in. PASS
- [x] Released: docs, man page, roadmap (C3 continues with Flatpak/Snap in v1.9.0), tag, GitHub, AUR (`options=('!debug')`)

## Next up — in this order

- [ ] **F-13 #49 (found 2026-10-08):** `nog remove` (-Rs) quietly takes along a dependency another installed app uses *optionally* (Elisa → `vlc-plugin-ffmpeg`, VLC would have lost most formats); no way to keep it. Fix: warn in plain words + ask Keep, and a `nog keep <pkg>` command (= `pacman -D --asexplicit`); test in the failing direction. Follow-up in nogForge #27
- [x] (1.8.0) AUR recipe: `options=('!debug')` — makepkg tries a debug package for nog and prints "No debugging symbols" (seen in Javier's yay build of 1.7.0, 4 Oct); harmless noise

### 00 · Found in the KognogOS VM, 30 Sep — v1.5.6, released for the VM test
- [ ] **NEW 2026-10-01 · found by Javier on this desktop, nog 1.5.6:** after the warnings are listed and nog is ready to install, **the prompt to continue does not appear**. Last `nog update` in fish history: 08:35. To do: reproduce, find the cause, open an F-n issue. **Reproduce on the next day nog has something ready** (1 Oct: 71 updates pending, all held, so no prompt is reached): `script -q ~/Programs/nog/logs/prompt-bug.log -c "nog update"`, answer `n` where the question should be; "Cancelled — nothing was installed" means nog's own gate was hidden, "exited with status 1" means pacman's. Suspect: pacman's prompt through the stderr relay / CRLF translation (v1.5.3) right after the `ignoring package upgrade` warnings. **Narrowed 2026-10-01 evening:** on kognogos-dev, nog 1.5.6 `nog update` (recorded with `script`, 26 `ignoring package upgrade` warnings) showed BOTH prompts — "Begin the handoff?" and pacman's "Proceed?" — so the bug is not in nog 1.5.6 alone; look at what differs on the desktop (fish, the terminal, sudo's pty, the KDE askpass) (Related to the open matrix line above: pacman's own "Proceed?" visible, 7.3)
- [x] **#30 · priority-1** · `nog install` on a fresh install used the disc's old package lists and failed; now refreshes the safe way first (lists older than a day → tier-aware update → install). *Code done, 219 tests; untested in the VM*
- [x] **#31 · priority-1** · keys first: archlinux-keyring / chaotic-keyring never held, installed in their own step first; a missing key store is set up. *Code done; untested in the VM*
- [x] **VM test** — *Claude's run done 2026-10-02 (clean-install-3): 6 PASS, 2.3 deferred, findings F-1 to F-4 written into the matrix.  **F-2, F-3, F-4 fixed (`5cd68b8`, `3dc89cb`, 221 tests) and proven in the VM; release v1.5.7 next (fakeroot into the AUR depends=). F-1 waits on Javier publishing the key.** #30 and #31 work; two KognogOS gaps (F-1 nog's key unreachable, F-2 no fakeroot) block them on a fresh install. Next: the fixes (v1.5.7) (issues #33–#36 + KognogOS#9), then his own run.* `testing/20260930 - Test Matrix for nog v1-5-6.md`, once the hypeForge session is done with `kognog-hypeforge` (Javier first ruled: wait, no release; then asked for the AUR release to test it in the VM himself)

- [x] **v1.5.7 · Javier's own VM run** — done 2 Oct; raised F-5 to F-7, plus F-8 found while recording → all fixed in v1.5.8
- [x] **F-1 · #33** — Javier's ruling 2026-10-02: no outside key service. Every instruction now fetches the key from GitHub (`curl -s https://github.com/jetomev.gpg | gpg --import`): 5 AUR recipes, v1.5.7 notes, KognogOS `where-we-stand.md` (`ee253e3`). Tested in an empty key store. Shipping the key inside KognogOS stays with KognogOS#9

- [ ] **Follow-up of #40** — the AUR helper / Flatpak / Snap steps still relay stderr through nog; the helper asks questions, so its question could race its table the same way. Check on this desktop (yay)
- [ ] **Desktop: confirm the 1 Oct missing prompt is gone** with v1.5.8 (`nog update` on a day something is Ready) — *nog 1.5.8 installed on the desktop 2026-10-02 12:44 (by nog 1.5.6 `install <file>`, which first updated 15 Ready packages)*
- [ ] The "update completed" record is per user (`~/.local/share/nog/last-update`); root and javier each keep their own. Safe direction (extra update only); decide whether it should be system-wide

### 0 · Javier's concerns, raised 30 Sep (the "two-week topic") — in this order
- [x] **A · #27 · priority-1 · Holds that never end.** *Code done 30 Sep, v1.5.2: 191 tests; dry run on this desktop releases linux-zen + mesa, linux-lts 1 day into its safety wait.* The waiting period is counted from the build date of the *newest* version, so every new build restarts the clock. Proven on this desktop 30 Sep: linux-zen has been held since at least 29 Jul (7.0.5 installed; 9 different new versions came and went, the countdown never reached 0; the newest, 7.2.7, is 9 days old). Tier 1 waiting longer than its own 30 days: linux-zen (+headers), linux-lts (+headers, 12 versions), mesa, lib32-mesa, mkinitcpio. Tier 2 today: none. Tier 3: python-platformdirs (31 days, 7 versions). The logs start 29 Jul, so these are minimums. Needs a ruling on the fix, then an issue. **Ruled 30 Sep: count from the first new version + a safety wait (T1 7 days, T2 3, T3 1, settable)**
- [x] **F-1 of v1.5.2 · #29** · pacman's warnings printed as a staircase under sudo's `use_pty`; fixed in v1.5.3
- [x] **B · #28 · priority-2 · Say where every held/ready package comes from.** *Extended 30 Sep: a SUMMARY table (Source · Ready now · On hold · Total, one row per repository; `could not check` instead of 0; `off` for a deactivated source) replaces the four `reported by` lines. Ships with #25 + #26 as v1.5.4.* Today only Flatpak and Snap rows get a word in the Note column; AUR rows are not marked at all, and official ones don't say which repository. Wanted: a Source column on every table. Flatpak (v1.1.0) and Snap (v1.2.0) updates already work with holds; installing through them is the unbuilt part (#7). Installed here: 2 Flatpaks (Flatseal, Termius), 1 snap (hello)
- [x] **C · #25 and #26** — both in v1.5.4 with #28

### 1 · The next fix release — stop silent breakage
- [x] **#16 · priority-1** · *code done 29 Sep, `51ac397`; not released yet* · A program can break silently after an update when it uses a shared library it never declared (seen with OBS and a Blu-ray library). nog has to spot these hidden links and hold both sides together.
- [x] **#14 · priority-4, rides along** · *code done 29 Sep, `587ff28`; checked by reading only* · When nog runs from a script, the "continue?" question runs into the next line. It needs one missing line break.

### 2 · The run-log release — make the log tell the truth
One change to the log file fixes all four, so they ship together.
- [x] **#19 · priority-2** · *code done 29 Sep, `9e54083`* · The log says `installed` on packages nog deliberately held back.
- [x] **#20 · priority-3** · *code done 29 Sep, `9e54083`* · The log can't show whether a package came from pacman or from Snap.
- [x] **#21 · priority-3** · *code done 29 Sep, `9e54083`* · When a handoff fails, the log records the error number but not the reason.
- [x] **#22 · priority-3** · *code done 29 Sep, `9e54083`* · When nog advises a reboot, it leaves no trace in the log.

### 3 · Install our own packages through nog
- [x] **#17 · priority-2** · *code done 29 Sep, `01bbe94`* · `nog install` can't install a package file we built ourselves, so every Forge release falls back to raw pacman. It should accept a file and still show its tier.

### 4 · Prove nog works with paru — DONE 29 Sep
- [x] **#12** · nog works with paru: install, auto-fallback, update plan, dates, and paru's AUR build path all checked; yay removed and restored. Results in `testing/20260929 - Test Results for nog v1-5-1-paru.md`
- [x] **#25 (F-2) · priority-3** *(v1.5.4)* · a failing AUR helper reads as "no AUR updates", silently. Found during #12
- [x] **#26 · priority-3** *(v1.5.4)* · with no terminal attached, `nog install <aur-pkg>` dies inside yay's menu with no plain explanation. Refuse up front and say why; do NOT auto-answer (that skips the PKGBUILD review). Found 29 Sep releasing grubForge v1.1.2
- [ ] Still unseen: paru's "nothing to update" answer, and an AUR update handed to paru

### 5 · Cache cleanup
- [x] **#15 · priority-3** *(v1.5.5: pacman's cache; orphans, AUR build caches, Flatpak runtimes and Snap revisions stay in C5)* · `nog clean`: clear out old package downloads, but keep the ones a held package may still need.

### 6 · The v2 plan
- [ ] **#7 · priority-3** · nog manages Flatpak and Snap as well as pacman, gains a JSON output mode, and gets its own app (nogForge).

### 7 · Later
- [ ] **#32 · found 2026-10-01 (forgeKit session):** `nog install` has no way to answer the AUR helper's "Proceed?" for **repository** packages when run from a script (a piped `y` was needed for python-pyte). Proposal: `nog install --yes`, passed through to the helper; AUR builds keep refusing without a person (that refusal worked correctly the same day)
  - **Seen again 2026-10-07 (nog 1.8.0):** three runs from Claude's terminal-less shell (`install` ×2 of repository packages, `remove monique`) stopped at pacman's / yay's [Y/n], reported as "you answered no"; same line in a terminal worked. `remove` has the same gap; `nog install --help` shows no options. #48 filed then closed as a duplicate of #32
- [ ] **#5 · priority-4** · A rare mismatch: a newer app built against a newer system library than the one nog is still holding back.

---

## Housekeeping
- [x] Every open issue has a priority label (29 Sep 2026)
- [x] This TODO file created (29 Sep 2026)
