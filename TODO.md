# nog — the list

**Current release: v1.4.1** (16 Sep 2026). 135 tests pass. Installed on this desktop and live on the AUR.
nog is the package updater for KognogOS. It holds new packages back for a waiting period set by their tier, so a bad update has time to be noticed before it reaches this computer.

**Updated after every step.** The full story behind each item is in its GitHub issue.

---

## Next up — in this order

### 1 · The next fix release — stop silent breakage
- [ ] **#16 · priority-1** · A program can break silently after an update when it uses a shared library it never declared (seen with OBS and a Blu-ray library). nog has to spot these hidden links and hold both sides together.
- [ ] **#14 · priority-4, rides along** · When nog runs from a script, the "continue?" question runs into the next line. It needs one missing line break.

### 2 · The run-log release — make the log tell the truth
One change to the log file fixes all four, so they ship together.
- [ ] **#19 · priority-2** · The log says `installed` on packages nog deliberately held back.
- [ ] **#20 · priority-3** · The log can't show whether a package came from pacman or from Snap.
- [ ] **#21 · priority-3** · When a handoff fails, the log records the error number but not the reason.
- [ ] **#22 · priority-3** · When nog advises a reboot, it leaves no trace in the log.

### 3 · Install our own packages through nog
- [ ] **#17 · priority-2** · `nog install` can't install a package file we built ourselves, so every Forge release falls back to raw pacman. It should accept a file and still show its tier.

### 4 · Prove nog works with paru
- [ ] **#12 · priority-3** · nog has claimed to support paru (another AUR helper) since v1.0.0 and has never been tested with it. Install paru, run the full test list, then do the same with yay again. This must happen before nogForge is built.

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
