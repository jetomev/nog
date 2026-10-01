# nog — the list

**Current release: v1.5.6** (30 Sep 2026) — fresh package lists before an install (#30), keys never held (#31). 219 tests. On GitHub (Latest, with the ready-built package attached for the VM) and the AUR (`0aed844`). VM checks pending. Same day: v1.5.5 (#15), v1.5.4, v1.5.3, v1.5.2.
nog is the package updater for KognogOS. It holds new packages back for a waiting period set by their tier, so a bad update has time to be noticed before it reaches this computer.

**Updated after every step.** The full story behind each item is in its GitHub issue.

---

## Done — v1.5.0 released (29 Sep 2026)
v1.5.0 on GitHub (Latest, signed) and the AUR, installed on this desktop through its own `nog install <file>`. Tags v1.4.2 and v1.4.3 ride along. Issues #14, #16, #17, #19–#22 closed. 172 tests, 6 warnings.

## Still open from this release
- [x] **#24 (F-1)** — fixed and shipped as v1.5.1; installed on this desktop with `nog install <file>` (17:3x)
- [x] Fresh install from the AUR through yay — works (Javier, 17:15)
- [ ] Watch the first real update that installs something: pacman's own "Proceed?" visible, `installed` rows in the log (matrix 7.3, 7.4)
- [ ] Not seen yet on real data: the library scan firing (2.7), the reboot log (4.10), the #14 prompt line (3.1)
- [x] AUR search index caught up to 1.5.0-1 (checked 29 Sep, ~17:30)
- [x] README AUR badge shows 1.5.1 on github.com (checked through GitHub's own image cache, ~18:10)

## Next up — in this order

### 00 · Found in the KognogOS VM, 30 Sep — v1.5.6, released for the VM test
- [ ] **NEW 2026-10-01 · found by Javier on this desktop, nog 1.5.6:** after the warnings are listed and nog is ready to install, **the prompt to continue does not appear**. Last `nog update` in fish history: 08:35. To do: reproduce, find the cause, open an F-n issue. **Reproduce on the next day nog has something ready** (1 Oct: 71 updates pending, all held, so no prompt is reached): `script -q ~/Programs/nog/logs/prompt-bug.log -c "nog update"`, answer `n` where the question should be; "Cancelled — nothing was installed" means nog's own gate was hidden, "exited with status 1" means pacman's. Suspect: pacman's prompt through the stderr relay / CRLF translation (v1.5.3) right after the `ignoring package upgrade` warnings (Related to the open matrix line above: pacman's own "Proceed?" visible, 7.3)
- [ ] **#30 · priority-1** · `nog install` on a fresh install used the disc's old package lists and failed; now refreshes the safe way first (lists older than a day → tier-aware update → install). *Code done, 219 tests; untested in the VM*
- [ ] **#31 · priority-1** · keys first: archlinux-keyring / chaotic-keyring never held, installed in their own step first; a missing key store is set up. *Code done; untested in the VM*
- [ ] **VM test** — `testing/20260930 - Test Matrix for nog v1-5-6.md`, once the hypeForge session is done with `kognog-hypeforge` (Javier first ruled: wait, no release; then asked for the AUR release to test it in the VM himself)

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
- [ ] **#5 · priority-4** · A rare mismatch: a newer app built against a newer system library than the one nog is still holding back.

---

## Housekeeping
- [x] Every open issue has a priority label (29 Sep 2026)
- [x] This TODO file created (29 Sep 2026)
