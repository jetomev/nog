# Test Results — nog v1.5.2

**Date:** 2026-09-30 · **Matrix:** [20260930 - Test Matrix for nog v1-5-2.md](20260930%20-%20Test%20Matrix%20for%20nog%20v1-5-2.md)
**Binary:** nog 1.5.2-1, installed on the development desktop with `nog install <file>`.

## The first real update with the fix (matrix 4.3) — Javier, 15:39

`nog update` released exactly what the dry run predicted: linux-zen + headers 7.0.5 → 7.2.7
(`waiting since Jul 29 · 8 newer versions skipped`), mesa + lib32-mesa, claude-desktop and five
Tier 3 packages. pacman installed all 10. The NVIDIA DKMS module was removed for 7.0.5 and built
for 7.2.7 (`dkms status`: `nvidia/615.71.09, 7.2.7-zen1-1-zen: installed`, all five modules
present). The initramfs was rebuilt. nog advised a reboot: `verified — linux-zen: running
7.0.5-zen1-1-zen, installed 7.2.7.zen1-1`. linux-lts 6.18.29 stayed as the fallback, held one
more day by its safety wait, its own NVIDIA module intact.

**#27 works in the field.**

## Findings

### F-1: pacman's warnings print as a staircase during `nog update`

**Seen:** every `warning: <pkg>: ignoring package upgrade` line started in the column where the
previous one ended, drifting across the screen, and `resolving dependencies...` started mid-line
after the last one. 59 lines, unreadable. Javier: *"the output of the warnings its terrible now."*

**Cause:** sudo 1.9.14+ runs the command in its own pseudo-terminal (`use_pty`, on by default —
this desktop has sudo 1.9.17p2) and puts the user's terminal in raw mode while it runs. stdout
passes through sudo's pty and gets carriage returns added. Since v1.4.3 (#21) nog pipes pacman's
**stderr** through its own relay to keep the reason for a failure; that pipe bypasses sudo's pty,
so each bare line feed reached a raw terminal: down one row, no return to the left edge.

**Why it was not caught earlier:** the v1.5.0 matrix check 7.3 ("watch the first real update that
installs something") was still open. This was that update. The relay's tests used an in-memory
sink, which cannot be in raw mode.

**Impact:** display only. Nothing was installed or skipped wrongly; the run log is unaffected.

**Fix:** when nog's stderr is a terminal, the relay sends every bare `\n` as `\r\n` (a CR before
the LF is invisible in a terminal's normal mode). A file or pipe gets the bytes untouched.
Tests fail with the fix disabled (2 of 10 handoff tests) and pass with it. Ships in v1.5.3.

### Not nog: claude-desktop's install script mentions Postman

`strings: 'opt/postman/app/postman': No such file` and `WARNING: Postman uses .` came from
`claude-desktop`'s own `.INSTALL` script (chaotic-aur build 2.9939.4-1), lines 6–7: a leftover
from a Postman package. Harmless; the packager's bug, not reported yet.
