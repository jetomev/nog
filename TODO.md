# nog — the list

**Current release: v1.5.1** (29 Sep 2026). 172 tests pass. Installed on this desktop and live on GitHub and the AUR.
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
- [ ] **#25 (F-2) · priority-3** · a failing AUR helper reads as "no AUR updates", silently. Found during #12
- [ ] Still unseen: paru's "nothing to update" answer, and an AUR update handed to paru

### 5 · Cache cleanup
- [ ] **#15 · priority-3** · `nog clean`: clear out old package downloads, but keep the ones a held package may still need.

### 6 · The v2 plan
- [ ] **#7 · priority-3** · nog manages Flatpak and Snap as well as pacman, gains a JSON output mode, and gets its own app (nogForge).

### 7 · Later
- [ ] **#5 · priority-4** · A rare mismatch: a newer app built against a newer system library than the one nog is still holding back.

---

## Housekeeping
- [x] Every open issue has a priority label (29 Sep 2026)
- [x] This TODO file created (29 Sep 2026)
