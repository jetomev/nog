# Test plan — nog v1.5.6 in the KognogOS VM (#30, #31)

**Why here:** #30 was found on a fresh KognogOS install (the hypeForge edition, kognog `d30a082`). A fix is believed only where the bug lives.
**When:** after the hypeForge session is done with the VM `kognog-hypeforge` (Javier's ruling, 30 Sep 2026). **No v1.5.6 release before this passes.**
**Binary:** `target/release/nog` from commit "feat(install): fresh package lists before an install, keys first", copied into the VM, not installed.

## Starting point
Best: revert `kognog-hypeforge` to the snapshot `clean-install-2` (a fresh install: the disc's own package lists, never refreshed). Ask the hypeForge session first; that revert discards whatever the VM holds now.

| # | Check | Expected | Result | Notes |
|---|---|---|---|---|
| 1.1 | Reproduce with the installed (old) nog: `nog install cowsay` | fails: files not found on the mirrors | **DEFERRED** | the failing direction first |
| 1.2 | New nog: `/tmp/nog install cowsay` | "the package lists are N days old — refreshing them, the safe way", a tier-aware update, then cowsay installs | **DEFERRED** | |
| 1.3 | Same, with everything held | lists refreshed with `pacman -Sy`, then the install | **DEFERRED** | may not occur naturally |
| 1.4 | Answer **n** at "Begin the handoff?" | install stops: "the update did not complete … not installing cowsay" | **DEFERRED** | |
| 1.5 | Run again right away | lists fresh: installs at once, no update | **DEFERRED** | |
| 2.1 | A pending `archlinux-keyring` | Ready, `keys · never held, installed first`; its own pacman step before the main one | **DEFERRED** | a fresh install from an older disc usually has one pending |
| 2.2 | Key store missing (`/etc/pacman.d/gnupg` moved aside) | nog explains, runs `pacman-key --init` + `--populate`, carries on | **DEFERRED** | restore the original afterwards |
