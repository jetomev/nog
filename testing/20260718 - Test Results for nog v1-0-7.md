# nog — v1.0.7 dogfood test results

Focused output-reformat release: `nog update` rebuilt around a banner header, per-source counts, and three aligned tables (READY TO INSTALL / ON HOLD FROM INSTALL / UNKNOWN), plus a pre-handoff `Proceed?` gate. No behavior change to the hold/tier engine — the regression surface is the new pure `format_table()` (2 new unit tests), which runs in the AUR build's `check()` on every install.

## Run metadata

- **Package under test:** `nog 1.0.7-1`, built + installed via `makepkg -si` from the AUR clone, then pushed to the AUR.
- **AUR source:** `aur.archlinux.org/nog.git` commit `560829f nog 1.0.7 — reformatted nog update output`
- **GitHub source:** `jetomev/nog` tag `v1.0.7` → commit `661b908 docs: version bump + changelog/roadmap for v1.0.7`
- **Tarball sha256:** `efb9cbf804545311411bc08393491f29e6ec3d50b6078317167b89e490cac7a8` (verified by makepkg from the GitHub tag tarball)
- **`check()` phase:** `cargo test --release --locked` → **35 passed, 0 failed** during the AUR build (was 33; +2 for `format_table`)
- **Test run:** 2026-07-18, on the KognogOS desktop (Ryzen 7 7700, 31 GiB)
- **Tester:** Javier (`jetomev`) + Claude (Anthropic), per [[nog-development-discipline]] and [[github-surface-completeness]]
- **Trigger:** a user-driven redesign of the `nog update` report — from ASCII-list buckets to header + per-source counts + aligned tables.

## The live render

`nog update` on the real pending set (200 official + 1 AUR), non-interactive stdin so the `Proceed?` gate hits EOF and cancels cleanly (no sudo, no handoff):

```
nog - Update!
=============
Date: 07/18/2026
Time: 02:12 PM
User: jetomev

nog: Checking for pending updates ...

nog: 200 official repository update(s) reported by pacman.
nog: 1 AUR update(s) reported by yay.

READY TO INSTALL:
-----------------

Package (2)            Old Version  New Version  Tier  Note

gnupg                  2.4.9-1      2.4.9-2      3     hold just expired
python-markdown-it-py  4.0.0-2      4.2.0-1      3     5 days past window

ON HOLD FROM INSTALL:
---------------------

Package (199)                Old Version    New Version    Tier  Note
alsa-card-profiles           1:1.6.7-1      1:1.6.8-1      2     6 days remaining
...
lib32-nvidia-utils           610.43.02-1    610.43.03-1    3     coupled to nvidia-utils · 2 days

UNKNOWN:
--------

(none)

nog: Proceed with installation? [Y/n] nog: Cancelled — nothing was installed.
```

## What this verifies

| Assertion | Evidence | Result |
|---|---|---|
| **Banner header** | name / Date / Time (from system `date`) / User rendered | ✅ |
| **Per-source counts** | separate official (pacman) + AUR (helper) lines with correct counts | ✅ |
| **Three aligned tables** | Package(N) / Old / New / Tier / Note columns line up; count in each header | ✅ |
| **Tier as colored digit** | bare `2`/`3` under a `Tier` header, per-tier color (padding outside the ANSI, alignment intact) | ✅ |
| **Empty section** | UNKNOWN with no rows renders `(none)` | ✅ |
| **v1.0.6 coupling still correct** | `lib32-nvidia-utils … coupled to nvidia-utils · 2 days` in the Held table | ✅ |
| **Proceed gate** | `Proceed? [Y/n]`; EOF (non-interactive) → clean cancel, no handoff | ✅ |
| **`format_table` unit tests** | alignment + `(none)` cases green in `check()` | ✅ |

## Verdict

**No findings.** The reformat matches the locked design, the hold/tier/coupling engine is unchanged and still correct in the output, and the pure renderer is unit-tested. Terminal width intentionally ignored (long `gcc`-family version strings widen the columns, as designed).

**Deferred to v1.0.8:** the per-run **CSV log + 3-month retention** (the closing `For full details check log …` lines land with it).
