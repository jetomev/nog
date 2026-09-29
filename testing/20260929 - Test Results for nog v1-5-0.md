# Test Results — nog v1.5.0

**Date:** 2026-09-29 · **Scope:** v1.5.0 (#17), carrying v1.4.2 (#16, #14) and v1.4.3 (#19–#22)
**Matrix:** [20260929 - Test Matrix for nog v1-5-0.md](20260929%20-%20Test%20Matrix%20for%20nog%20v1-5-0.md) — the full table of checks.

## Roll-up (from `tally-matrix.py`, not by hand)

37 checks · **29 PASS** · 4 FAIL, all fixed before release (M-1, M-2, M-3, plus 1.5 which is M-1) · **4 CANNOT TEST**

## What happened, in order

1. Unit and live checks on the dev build (§1–§6). Three documentation findings, fixed in the v1.5.0 docs commit.
2. GitHub: `main` and tags v1.4.2, v1.4.3, v1.5.0 pushed; v1.5.0 Release published with four signed assets. The tarball reproduces from `git archive` and verifies against the release key.
3. AUR: PKGBUILD bumped, `updpkgsums`, `.SRCINFO` regenerated; signature, checksum and `.SRCINFO` pre-flight clean. `makepkg` smoke build from the signed asset: 172 tests pass.
4. AUR push `03ca0dc` (after Javier unlocked the AUR key in an ssh-agent).
5. **Javier installed v1.5.0 with `nog install <file>` — the release installed by its own new feature.**
6. The installed binary ran a real `nog update`: 54 pending, all held, all logged `held` with the right source.

## Not proven (4)

| Check | What it needs |
|---|---|
| 2.7 — the library scan firing in a real update | a pending update that drops a library version |
| 3.1 — the #14 prompt line | a source step failing |
| 4.10 — a real reboot-log row | a Tier 1 reboot-class package installing |
| 7.3 — pacman's own `Proceed?` through the relay | an update with Ready packages. Covered meanwhile by the timing test, proven in the failing direction |

## Findings

| ID | Severity | Description | Status |
|---|---|---|---|
| F-1 | low | A test prints a fake `error: no space left` into every package build log. [#24](https://github.com/jetomev/nog/issues/24) | OPEN — next batch |
| M-1 | low | Man page column list could not wrap (from v1.4.3) | FIXED |
| M-2 | medium | Privilege sections wrong about when `update` uses `sudo pacman` (since v1.3.0) | FIXED |
| M-3 | low | v2 design doc's version column stale | FIXED |

**Process note.** The v1.4.2 and v1.4.3 tags ran the tests but not the man-page formatting gate, so
M-1 rode one tag before the checklist caught it. A tag between releases gets the same audit greps
as a release.
