# nog release checklist

Pre-flight gates that every nog release (Phase wrap or hotfix) must pass before tag + GitHub release + AUR push. Captures process gaps surfaced by past releases and the v1.0.3 incident response. Apply per [[nog-development-discipline]] and [[github-surface-completeness]].

## Version sync — all of these must agree

Before tagging, every surface below must carry the new version string `vX.Y.Z`:

- `Cargo.toml` `version`
- `Cargo.lock` `[[package]] name = "nog"` `version` (updates automatically via `cargo check`)
- `config/nog.conf` `[general] version`
- `nog.1` `.TH NOG 1 "Month Year" "nog vX.Y.Z"` header
- `README.md` Version badge URL
- `README.md` nog.conf example `version = "X.Y.Z"`
- `PKGBUILD` (in-tree) `pkgver`
- `~/Programs/aur-nog-remote/PKGBUILD` `pkgver` + `pkgrel`
- `~/Programs/aur-nog-remote/.SRCINFO` (regenerate via `makepkg --printsrcinfo`)
- The annotated tag message references the version

Quick audit:

```bash
grep -E "^version = |pkgver=|nog v[0-9]" Cargo.toml config/nog.conf nog.1 PKGBUILD README.md | head -20
```

## Doc coverage

- Man page **COMMANDS** section lists every public subcommand declared in `src/main.rs` `Commands` enum (excluding `_debug-*` hidden variants)
- Man page **TIER SYSTEM** section names every default Tier 1 and Tier 2 package (must match `config/tier-pins.toml`)
- Man page **TROUBLESHOOTING** section exists for any known failure mode that needs user action (v1.0.3+: kernel/headers desync)
- README **Features** list, **Tier system** subsections, **Usage examples**, and **Configuration** keys all reflect current behavior
- README **Troubleshooting** section mirrors the man-page troubleshooting entries
- README **Changelog** leads with the latest release; previous entries are not altered
- README **Roadmap** marks the just-shipped phase as ✅ shipped

## Audit greps

Must run from the repo root before tagging:

```bash
# 1. Tests green and locked
cargo test --release --locked

# 2. No leftover dev scaffolding
grep -rn "TODO\|FIXME\|XXX" src/ | grep -v "^Binary"

# 3. Release binary has no embedded maintainer paths (F2 regression guard from v1.0.2)
strings target/release/nog | grep -i CARGO_MANIFEST_DIR
# MUST be empty

# 4. Warning delta — note in changelog if non-zero from last release
cargo build --release 2>&1 | grep "^warning:" | wc -l
```

## Co-author credit

Every release artifact carries the dual credit line:

- `PKGBUILD` `# Co-developer: Claude (Anthropic)`
- `~/Programs/aur-nog-remote/PKGBUILD` same line
- `README.md` Authors / Credits section
- Man page **AUTHORS** section
- GitHub release body footer

## Release-day flow

1. Version sync ✓ (greps above)
2. Audit greps ✓
3. `cargo test --release --locked` ✓
4. README sweep top-to-bottom — every section accurate, all in-repo links resolve
5. `testing/` folder current:
   - This release's Test Matrix renamed/copied to `YYYYMMDD - Test Matrix for nog v<X-Y-Z>.md` (or kept as the rolling current matrix with date updated)
   - Prior release's Test Results preserved under its original name
   - `RELEASE-CHECKLIST.md` updated if new gates surfaced
6. Phase N implementation commit + docs/version-bump commit land on `main`
7. Annotated tag `vX.Y.Z` on the docs commit (or rolled forward if deeper docs follow)
8. **GitHub push (before AUR — PKGBUILD `source=()` pulls the tag tarball):**
   - `git push origin main`
   - `git push origin vX.Y.Z`
   - `gh release create vX.Y.Z --title "..." --notes-file <body>`
9. **AUR pipeline:**
   - Bump `~/Programs/aur-nog-remote/PKGBUILD` `pkgver` (reset `pkgrel=1`)
   - `cd ~/Programs/aur-nog-remote && updpkgsums && makepkg --printsrcinfo > .SRCINFO`
   - Mirror the same `PKGBUILD` into `~/Programs/aur-nog/` for the local smoke test
   - In `~/Programs/aur-nog/`: `makepkg -si` (sudo handoff to user — copy-paste, paste result back)
   - In `~/Programs/aur-nog-remote/`: `git add PKGBUILD .SRCINFO && git commit -m "vX.Y.Z" && git push`
10. **Fresh install verification:**
    - `sudo pacman -R nog` (handoff)
    - `yay -S nog` (handoff)
    - `nog --version` matches the tag
    - Section 1 (Baseline sanity) of the current Test Matrix passes
11. **GitHub surface review** per [[github-surface-completeness]]:
    - About — description, topics, website URL all current
    - Releases — v1.0.X marked Latest, body matches the prepared notes, prior releases unaltered
    - Packages — AUR link still resolves
    - README cross-links resolve on github.com (render check)
    - **AUR badge cache check (shields.io ↔ camo):**
      - Verify shields.io is current: `curl -s 'https://img.shields.io/aur/version/nog' | grep -oE 'v[0-9.]+'` should print the new version
      - Verify github.com renders current: load the repo page; if the AUR badge still shows the old version 15+ minutes after the AUR push, GitHub's camo proxy is holding a stale SVG. **Fix:** bump a query param on the badge URL to bust the camo hash. The cheapest cache-buster is `?color=1793d1` (Arch blue) which is already in place; if needed again, change to `?color=1793d2` (or any new value), commit, push. New URL → new camo hash → fresh fetch from shields.io.
      - Root cause for the record: camo caches by source-URL hash with `max-age=3600`. If shields.io's own AUR API cache was still stale when camo first fetched after a push, camo locks that stale value in for up to an hour. URL change is the only deterministic refresh.
12. If any finding surfaces during steps 10–11, batch as `Fn`/`Mn` items and write a new Test Results file in `testing/`. Hotfix batch becomes the next release (vX.Y.Z+1).

## After release — record-keeping

- New `testing/YYYYMMDD - Test Results for nog v<X-Y-Z>.md` if any findings (skip if perfectly clean)
- Memory: update `[[nog]]` snapshot if the release shifts the architecture or surface
- End-of-day session log to the vault if a multi-session push
