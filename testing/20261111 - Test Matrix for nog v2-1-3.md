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

**Resume here (updated 2026-11-11, 21:15).** The package is built by makepkg from the published GitHub release in `~/Programs/aur-nog/build-213/` (the tests ran inside the build; its checksum and signature checked). Javier said yes to the AUR at 20:47; the push of **2.1.3-1** waits only on this retry passing. State before it (the Council's repair, 20:56): simdjson 1:5.0.5 back from the cache and `nog lock`ed, node works; libmupdf 1.28.5-2 back and `nog lock`ed (20:57). zathura's PDF plugin is still broken for another reason, gumbo (F-33, [#70](https://github.com/jetomev/nog/issues/70)): `libgumbo.so.4` comes with gumbo-parser 0.14.1, which nog has held for 38 days as `blocked by litehtml0.9`, an orphan nothing on this computer uses. Step 4 removes it, so the update in step 6 brings gumbo-parser too.

Hand Javier one step at a time, in plain words: what it does, the one command, what he should see, then wait for his pasted output. Deadline: before the monthly round, **Saturday 2026-11-14, 9 AM**.

1. Install: `nog install ~/Programs/aur-nog/build-213/nog-2.1.3-1-x86_64.pkg.tar.zst` — swaps 2.1.2 for 2.1.3, nothing else.
2. Version: `nog --version` → `nog 2.1.3`.
3. `nog unlock simdjson libmupdf` — nog's new rule now does what the two locks did.
4. `nog remove litehtml0.9 litehtml0.9-debug` — the orphan (nothing needs it, no program links it); only those two go.
5. Look first: `nog update`, answer **n**. Held: simdjson `coupled to nodejs`, libmupdf `coupled to zathura-pdf-mupdf`. Ready: gumbo-parser 0.13.2-1 → 0.14.1-1.
6. The real run: `nog update`, answer **y**. Finishes, no "conflicting files".
7. `node --version` prints `v26.12.1`.
8. `ldd /usr/lib/zathura/libpdf-mupdf.so | grep "not found"` prints nothing (zathura reads PDFs again).
9. Then Claude runs §4: the damage check over his run (every program and plugin against every library file it removed), fills the Result columns, tallies with `tally-matrix.py`.
10. Only on his pass: the AUR push of **2.1.3-1** from `~/Programs/aur-nog-remote/` (one push: 2.1.1 and 2.1.2 were never pushed and are folded in; ssh agent per `~/.claude/rules/aur.md`; pre-flight pkgdesc ≤ 255 bytes) → close #67, #68 and #69 with the story → kognogos-dev gets 2.1.3 before its round (his command, prepared by Claude) → kognogos.org card + Vault note. #70 stays open (a later version).

A finding during his test goes into the same unpublished 2.1.3 (rebuild, re-test), never a second AUR push.

| # | Check | Expected | Result | Notes |
|---|---|---|---|---|
| 3.1 | `nog install ~/Programs/aur-nog/build-213/nog-2.1.3-1-x86_64.pkg.tar.zst` | asks his password, then only nog changes (2.1.2 → 2.1.3); no `.sig` question | | |
| 3.2 | `nog --version` | `nog 2.1.3` | | |
| 3.3 | `nog unlock simdjson libmupdf` | both locks gone | | |
| 3.4 | `nog remove litehtml0.9 litehtml0.9-debug` | only those two removed | | |
| 3.5 | `nog update`, answer **n** | simdjson Held `coupled to nodejs`; libmupdf Held `coupled to zathura-pdf-mupdf`; gumbo-parser Ready; answering n changes nothing | | |
| 3.6 | `nog update`, answer **y** | the run finishes (`Done · update`), no "conflicting files" | | |
| 3.7 | `node --version` | `v26.12.1` | | |
| 3.8 | `ldd /usr/lib/zathura/libpdf-mupdf.so \| grep "not found"` | prints nothing | | |

## §4 · Claude, after Javier's install

| # | Check | Expected | Result | Notes |
|---|---|---|---|---|
| 4.1 | the damage check over his 3.6 run (old packages from the cache, every installed ELF) | no program or plugin asks for a removed library file | | |
| 4.2 | the same check after Saturday's round | the same | | |

## Findings

*(none yet)*
