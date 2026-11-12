# Test Matrix for nog v2.1.3

*Built 2026-11-11, the evening Javier's run of 2.1.2 found F-32, so the fix is in his hands before the monthly update round (Saturday 2026-11-14, 9 AM, this desktop and kognogos-dev). §1–§2 are Claude's; §3 is Javier's, on this desktop, with the locally built AUR package; §4 is Claude's check after his install. The roll-up is `tally-matrix.py`'s, never a hand count.*

**What this version adds:** a Ready library package that declares no library versions (simdjson, libmupdf) waits for a held program rebuilt for its new version (`coupled to nodejs`). Everything 2.1.1 and 2.1.2 added stays.

**Where it came from:** [the 2.1.2 sheet](20261111%20-%20Test%20Matrix%20for%20nog%20v2-1-2.md), row 3.4: simdjson took `libsimdjson.so.34` from the held nodejs (`node` would not start), libmupdf took `libmupdf.so.28.5` from the held zathura-pdf-mupdf. Issue [#69](https://github.com/jetomev/nog/issues/69).

## §1 · Baseline sanity

| # | Check | Expected | Result | Notes |
|---|---|---|---|---|
| 1.1 | `cargo test --release --locked` | green | **PASS** | 327 passed, 0 failed, 6 ignored (319 at v2.1.2); the simdjson end-to-end test seen failing with the rule switched off |
| 1.2 | Warning delta vs v2.1.2 | unchanged | **PASS** | 6 → 6 |
| 1.3 | `grep -rn "TODO\|FIXME" src/` | empty | **PASS** | |
| 1.4 | `strings target/release/nog \| grep CARGO_MANIFEST_DIR` | empty | **PASS** | |
| 1.5 | `man --warnings -l nog.1` | no warnings | **PASS** | the new section included |
| 1.6 | Version sync | 2.1.3 in Cargo.toml, Cargo.lock, nog.conf, nog.1, README | **PASS** | |

## §2 · Claude, this desktop — read only

| # | Check | Expected | Result | Notes |
|---|---|---|---|---|
| 2.1 | The damage of the 20:34 run: every program and plugin against every library file it removed | the breaks named, nothing guessed | **PASS** | exactly two: `/usr/bin/node` ← `libsimdjson.so.34`, `/usr/lib/zathura/libpdf-mupdf.so` ← `libmupdf.so.28.5` (`logs/damage-20261111-2034-run.log`) |
| 2.2 | `scripts/replay-f32.py` over the 20:34 run (the old packages from the cache, the held programs on disk) | both breaks held; the cost listed by name | **PASS** | 7 of 180 Ready would wait: simdjson → nodejs, libmupdf → zathura-pdf-mupdf (the two breaks); libraw, openexr, smbclient, qt6-webengine, lib32-libx11 kept their number and would have waited needlessly (`logs/replay-f32-*.log`) |
| 2.3 | `scripts/dry-run-213.sh` (the installed 2.1.2 beside the build) | the seven and the dotnet pair held; nothing moves the wrong way; OK | **PASS** | nothing Ready on this desktop after the 20:34 run, so simdjson is not testable live here until §3.3 |

## §3 · Javier, this desktop — step by step

**Resume here (written 2026-11-11).** The package is built by makepkg from the published GitHub release in `~/Programs/aur-nog/build-213/` (the tests ran inside the build; its checksum and signature checked). Tonight's repair comes first and is the Council's (simdjson 1:5.0.5-1 and libmupdf 1.28.5-2 back from pacman's cache with `nog install`, each `nog lock`ed). Hand Javier one step at a time, in plain words: what it does, the one command, what he should see, then wait for his pasted output. Deadline: before the monthly round, **Saturday 2026-11-14, 9 AM**.

1. Install: `nog install ~/Programs/aur-nog/build-213/nog-2.1.3-1-x86_64.pkg.tar.zst` — swaps 2.1.2 for 2.1.3, nothing else.
2. Version: `nog --version` → `nog 2.1.3`.
3. The live test of the new rule: `nog unlock simdjson`, then `nog update`, answer **n**. simdjson shows in the Held table, `coupled to nodejs`. nog now does what the lock did, so the lock can stay off. Same for libmupdf (`coupled to zathura-pdf-mupdf`).
4. `node --version` prints a version (the repair holds).
5. Then Claude runs §4, fills the Result columns, tallies with `tally-matrix.py`.
6. Only on his pass: the AUR push of **2.1.3-1** from `~/Programs/aur-nog-remote/` (one push: the 2.1.2 commit there is folded into it, never pushed; ssh agent per `~/.claude/rules/aur.md`; pre-flight pkgdesc ≤ 255 bytes) → close #67, #68 and #69 with the story → kognogos-dev gets 2.1.3 before its round (his command, prepared by Claude) → kognogos.org card + Vault note.

A finding during his test goes into the same unpublished 2.1.3 (rebuild, re-test), never a second AUR push.

| # | Check | Expected | Result | Notes |
|---|---|---|---|---|
| 3.1 | `nog install ~/Programs/aur-nog/build-213/nog-2.1.3-1-x86_64.pkg.tar.zst` | asks his password, then only nog changes (2.1.2 → 2.1.3); no `.sig` question | | |
| 3.2 | `nog --version` | `nog 2.1.3` | | |
| 3.3 | `nog unlock simdjson`, `nog unlock libmupdf`, then `nog update`, answer **n** | simdjson Held `coupled to nodejs`, libmupdf Held `coupled to zathura-pdf-mupdf`; answering n changes nothing | | |
| 3.4 | `node --version` | a version number, no "cannot open shared object file" | | |

## §4 · Claude, after Javier's install

| # | Check | Expected | Result | Notes |
|---|---|---|---|---|
| 4.1 | `NOG_NEW=/usr/bin/nog NOG_OLD=/usr/bin/nog scripts/dry-run-213.sh` | R2b simdjson Held, coupled to nodejs; OK | | |
| 4.2 | the damage check re-run over Saturday's round | no program asks for a removed library file | | |

## Findings

*(none yet)*
