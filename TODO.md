# nog — the list

**Current release: v1.4.1** (16 Sep 2026) — installed and on the AUR. **v1.5.0 is being released** (172 tests).
nog is the package updater for KognogOS. It holds new packages back for a waiting period set by their tier, so a bad update has time to be noticed before it reaches this computer.

**Updated after every step.** The full story behind each item is in its GitHub issue.

---

## Right now — v1.5.0 release (29 Sep 2026)
Tags v1.4.2 (#16, #14) and v1.4.3 (#19–#22) made; v1.5.0 (#17) carries the GitHub Release. 172 tests, 6 warnings.
- [x] Code, docs and version strings for all three tags
- [x] Test matrix: 36 checks — 25 pass, 4 failed and fixed (M-1, M-2, M-3), 7 waiting for a real run
- [ ] Push `main` and the three tags, then the GitHub Release for v1.5.0
- [ ] AUR: bump the PKGBUILD, `makepkg` smoke build, push
- [ ] Javier installs v1.5.0 — using `nog install <file>` itself (§7.2)
- [ ] Dogfood §7: pacman's own "Proceed?" question still visible during `nog update` (§7.3), real log rows (§7.4)
- [ ] Close #14, #16, #17, #19, #20, #21, #22 with explanations
- [ ] Not seen yet on real data: the library scan firing (2.7), the reboot log (4.10), the #14 prompt line (3.1)

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
