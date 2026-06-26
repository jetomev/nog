# nog v2 — `nog explain <pkg>`

**Status:** DESIGN DRAFT — captured 2026-06-24, while findings are fresh. Not yet a committed v2.0 feature; no code work has begun. This document exists so the thinking isn't lost between now and whenever the v2 cycle actually opens.

**Triggered by:** the [2026-06-20 dogfood findings](../testing/) — see also `~/Google Drive/Rullynastre/Nog/2026-06-24.md` for the full conversation context.

---

## The thesis

`nog update` currently reports the **what**:

```
libayatana-appindicator 0.5.94-1 -> 0.6.0-1  [Tier 3 · 459 days past window]
x265 4.1-1 -> 4.2-2  [Tier 3 · 374 days past window]
```

It does not report the **why**. The user sees "459 days past window" and is left with three questions:

1. Is something broken? (No.)
2. Did I do something wrong? (No.)
3. Should I act, or wait? (Depends — and the answer isn't visible.)

`nog explain <pkg>` would close that gap by classifying *what kind of block* is causing the long-tail anomaly. This turns nog from a *hold manager* into a *diagnostic tool*.

This is a v2-class feature — not a hotfix, not a small extension. It needs new data sources (AUR RPC, SOVERSION introspection, pacman db queries), a classifier with multiple strategies, and a new command surface. It's the right size for a v2.0 release.

---

## The findings that prompted this

From the 2026-06-20 dogfood session (a `nog update` run that triggered 145 upgrades after a long quiet period), two long-tail outliers surfaced:

### Finding 1 — `libayatana-appindicator 459 days past window`
- **Root cause:** AUR package `spotify` carried a versioned dependency pin `depends=('libayatana-appindicator<0.6.0')` in its PKGBUILD.
- **Why it cleared:** the spotify AUR PKGBUILD was eventually updated to drop the pin (or libayatana's 0.6.x ABI became compatible enough that the pin was relaxed).
- **Detection mechanism:** `pactree -r libayatana-appindicator` showed `└─spotify`; cross-referencing the AUR package's `depends=` field against the new repo version revealed the conflict.

### Finding 2 — `x265 374 days past window`
- **Root cause:** Arch-wide coordinated library transition. `x265 4.1 → 4.2` is a SOVERSION bump that required rebuilding FFmpeg + libheif + kfilemetadata + kpipewire and the entire KDE Plasma 6 stack downstream.
- **Why it cleared:** KDE Plasma 6.7 reached release readiness, which carried the coordinated rebuild of x265's downstream cone.
- **Detection mechanism:** `pactree -r x265` showed a massive downstream tree dominated by KDE Plasma 6 packages — no AUR packages on the critical path. The simultaneous appearance of x265 + ffmpeg (with `-2 → -6` pkgrel ladder = four rebuilds) + ~50 Plasma 6.6.5→6.7.0 packages in the same upgrade batch is the signature of a coordinated transition completing.

### Finding 3 — multi-package rebuild cluster cleared 2026-06-26 (~700-day outliers)

A second dogfood run six days after the first surfaced **another cluster of long-tail entries**, this time predominantly pure `pkgrel`-bump rebuilds rather than version bumps. The same Case B / Case C pattern, different upstream catalysts.

| Package | Days past window | Old → New | Bump shape |
|---|---|---|---|
| `cabextract` | 708 | `1.11-2 → 1.11-3` | pure rebuild (pkgrel only) |
| `vid.stab` | 706 | `1.1.1-2 → 1.1.1-3` | pure rebuild |
| `xvidcore` | 706 | `1.3.7-3 → 1.3.7-4` | pure rebuild |
| `libssh2` | 535 | `1.11.1-1 → 1.11.1-5` | **four rebuilds queued** (-2, -3, -4, -5 all skipped) |
| `lib32-libssh2` | 469 | `1.11.1-1 → 1.11.1-3` | mirror of above, multilib lag |
| `libdovi` | 329 | `3.3.2-1 → 3.3.2-2` | pure rebuild |
| `lib32-libffi` | 321 | `3.5.2-1 → 3.6.0-1` | minor version bump |
| `python-moddb` | 187 | `0.14.0-2 → 0.15.0-1` | minor version bump + grew dep tree (curl-impersonate, python-curl_cffi, python-pycurl, python-eventlet, python-gevent, others) |

**Notable substructure:** the three 706–708-day entries are *exactly* clustered — three packages all sitting at the same age suggests a **single distro-wide rebuild batch** released together (likely a shared toolchain or library transition that touched all three at once). This is a stronger Case C signature than Finding 2's x265 case, because the simultaneous-age signal is observable from the raw output alone without any downstream-cone analysis.

**Notable secondary effect:** the `python-moddb 0.14 → 0.15` upgrade quietly pulled in **17 new packages** to satisfy the new version's dependency tree (curl-impersonate + Python scraping stack). `nog explain` could surface this kind of "dependency surface expansion warning" as an optional v2.1+ enhancement — *"this upgrade will add N new packages to your system"* — distinct from the core blocker-classification mission but in adjacent territory.

**What this finding adds to the design thesis:**
1. **Pure pkgrel-bump outliers are common, not rare.** Three of the eight long-tail entries in this batch were pkgrel-only — meaning users see "708 days past window" for what is technically the *same package version* they already have, just rebuilt. The `nog explain` output should differentiate "your package needs a rebuild" from "your package needs a new version" since the user-facing risk is very different.
2. **Multi-package age-clustering is itself a Case C signal.** Three packages all at 706-708 days is unlikely to be coincidence — it's the fingerprint of a coordinated rebuild batch. The classifier should detect age-clustering across multiple packages and surface it as a single explanation: *"these N packages cleared together — likely the same upstream rebuild batch."*
3. **The pattern recurs reliably.** Two consecutive dogfood sessions, six days apart, both surfaced 5+ long-tail outliers with the same root-cause families. This isn't a one-time anomaly worth a special-case fix — it's a **recurring class of output** that deserves a first-class diagnostic command.

All eight 2026-06-26 outliers were **legitimate upstream blocks**, same as Findings 1 and 2. nog correctly held them at the appropriate tier; the gap to the user's understanding is what `nog explain` would close.

---

The three findings together establish that long-tail "X days past window" entries are **not noise and not bugs** — they are a normal, recurring product of nog's tier system honestly reporting upstream coordination delays. nog correctly reported the gap each time; the gap had a real upstream cause that nog had no way to surface. `nog explain` is the missing piece.

---

## Proposed taxonomy

`nog explain <pkg>` should classify holds into a small, exhaustive set of cases:

### Case A — `AurPin`
An installed AUR package has a versioned `depends=` constraint that blocks the upgrade.

**Detection:**
1. `pactree -r <pkg>` to find reverse dependents.
2. For each dependent in the AUR (i.e., not in core/extra/multilib/chaotic-aur), introspect its installed `depends=` field via `pacman -Qi <aur-pkg>` or by fetching its current PKGBUILD from the AUR RPC.
3. Look for a versioned constraint that the new repo version of `<pkg>` would violate.

**Confidence:** HIGH when found — this is a deterministic, falsifiable check.

**Output template:**
```
BLOCKED — held by AUR consumer with versioned pin
  Blocker: aur/spotify 1:1.2.x.x — depends=('libayatana-appindicator<0.6.0')
  Resolution: update spotify (check AUR for newer PKGBUILD) or remove the pin
```

### Case B — `SoversionTransition`
A shared library has bumped SOVERSION; consumers are not yet rebuilt against the new ABI.

**Detection:**
1. Read the installed library's actual SOVERSION (from filename suffix: `libx265.so.215`).
2. Read the new repo version's expected SOVERSION (from package metadata or by downloading and inspecting).
3. Check whether any installed packages still link against the old SOVERSION using `lsof | grep libX.so.OLD` or scanning ELF `NEEDED` entries.
4. If there are bound consumers AND repo has not yet shipped rebuilds for them, that's the block.

**Confidence:** MEDIUM-HIGH. SOVERSION detection isn't perfect (some libs don't follow conventions), but for well-behaved libraries it's reliable.

**Output template:**
```
BLOCKED — distro library transition in progress
  ABI change: libx265.so.215 -> libx265.so.216
  Pending rebuilds: extra/ffmpeg (still links libx265.so.215), extra/libheif
  Resolution: wait for distro coordination; no user action needed
```

### Case C — `ReleaseCycleCoordination`
A foundation library is held while a downstream ecosystem (KDE, GNOME, Mesa, etc.) prepares a coordinated major release.

**Detection:** harder. Heuristics:
1. `pactree -r` returns a large downstream cone (>20 packages).
2. The downstream cone is dominated by packages from a single release umbrella (recognized by package-name prefix: `plasma-*`, `gnome-*`, `kf6-*`, etc.).
3. Many of those downstream packages are *also* currently held or have been recently bumped together.
4. Optionally: cross-check release status of the umbrella project (KDE.org release page, GNOME release schedule) — but this is fragile and network-dependent.

**Confidence:** LOW-MEDIUM. This is a heuristic classifier, not a deterministic one. Output should hedge.

**Output template:**
```
PROBABLY BLOCKED — distro release-cycle coordination (heuristic)
  Downstream cone: 47 KDE Plasma 6 packages depend on x265
  Likely umbrella: KDE Plasma (current: 6.6.5, latest upstream: 6.7.0)
  Resolution: wait for the Plasma 6.7 release in Arch; no user action needed
```

### Case D — `UserIgnore`
The package is explicitly listed in `IgnorePkg` somewhere.

**Detection:**
1. Grep `/etc/pacman.conf` for `IgnorePkg`.
2. Grep `~/.config/yay/config.json` (or equivalent) for ignored packages.
3. Check `IgnoreGroup` overlaps.

**Confidence:** HIGH. This is just config parsing.

**Output template:**
```
BLOCKED — user ignore directive
  Source: /etc/pacman.conf line 42 — IgnorePkg = linux-zen
  Resolution: remove from IgnorePkg if you want updates
```

### Case E — `Unknown`
None of the above classifiers fired confidently.

**Output template:**
```
No specific blocker identified
  Days past window: 459
  Downstream consumers: 1 (aur/spotify) — checked, no pin found
  Possible causes: yay session ignore, transient PKGBUILD issue, partial state
  Suggested next step: 'pactree -r <pkg>' and 'pacman -Qi <pkg>' to investigate manually
```

Classifier order: D → A → B → C → E (cheapest/most-deterministic first).

---

## Proposed command surface

### `nog explain <pkg>`
Deep-dive on a single package. Default output: human-readable summary as templated above.

### `nog explain --all-blocked`
Loop over every currently-held package and run `explain` on each. Useful for "system health" checks.

### `nog explain <pkg> --tree`
Show the full `pactree -r` output alongside the classification, for users who want to verify the analysis.

### `nog explain <pkg> --json`
Machine-readable output for scripting / dashboards.

### `nog update --diagnose` *(optional integration)*
Append a one-line reason to each entry in the `nog update` output:

```
libayatana-appindicator 0.5.94-1 -> 0.6.0-1  [Tier 3 · 459 days past window — blocked by aur/spotify pin]
```

This is the highest-discoverability path. Trade-off: it slows down `nog update` (requires AUR RPC calls). Could be off by default, behind `--diagnose`.

---

## Data sources nog v2 needs

| Source | Purpose | Cost |
|---|---|---|
| `pactree -r <pkg>` (already a binary on every Arch system) | Reverse dependency tree for blocker hunting | Cheap — just shell out |
| `pacman -Qi <pkg>` | Installed version, install date, install reason, depends | Cheap — local DB |
| `pacman -Si <repo>/<pkg>` | Repo version, depends | Cheap — local sync DB |
| AUR RPC (`https://aur.archlinux.org/rpc/?v=5&type=info&arg[]=<pkg>`) | Current AUR PKGBUILD metadata (depends, version) | Network — needs caching + rate-limit awareness |
| ELF inspection (`readelf -d` or parsing NEEDED entries) | SOVERSION linkage detection | Medium — requires reading binaries |
| Pacman log (`/var/log/pacman.log`) | When was the last upgrade attempt? Did anything in this cohort recently update? | Cheap — local file |
| `/etc/pacman.conf` + `~/.config/yay/config.json` | User ignore directives | Cheap |

The AUR RPC is the only network dependency, and it's gated behind explicit `nog explain` invocation. `nog update` itself stays offline.

---

## Risks & open questions

1. **AUR RPC rate limiting.** If a user has 50 AUR packages and runs `nog explain --all-blocked`, that's 50 RPC calls. Need to: (a) batch via `arg[]=` multi-arg API (RPC supports up to ~200 packages per call), (b) cache responses for at least an hour, (c) honor any rate-limit headers if present.

2. **PKGBUILD parsing complexity.** `depends=` is bash array syntax. Need to handle: quoted strings, version operators (`<`, `<=`, `>=`, `=`), provides/conflicts/replaces, comments inline. Options: shell out to `pkgver` or `makepkg --printsrcinfo`, write a focused parser, or accept regex limitations.

3. **SOVERSION isn't always cleanly bumped.** Some libraries change ABI without bumping the SOVERSION (a bug, but it happens). Others bump SOVERSION for unrelated reasons. The Case B classifier should be honest about confidence levels.

4. **Release-cycle coordination is fundamentally fuzzy.** Case C is the most useful classifier for users (it explains the most "I'm not actually broken, am I?" anxiety), but it's also the most hand-wavy. Hedge the output. Don't claim certainty we don't have.

5. **Should this be `nog explain` or a separate binary (`nogdoctor`)?** Arguments both ways:
   - **In nog:** discoverable (`nog --help` lists it), shares tier-pins config, single install.
   - **Separate:** keeps nog binary lean, lets diagnostic features evolve at a different cadence, avoids growing nog's network dependency surface.
   - **Recommendation:** start in nog. If it grows large or develops a different release cadence, spin out.

6. **Test surface for Case B and Case C.** Hard to dogfood deterministically — depends on real-world distro coordination events. May need synthetic test fixtures or rely on opportunistic dogfooding when transitions happen.

7. **What goes in `nog update` by default?** The minimum-viable integration is `nog update --diagnose` (opt-in flag). The maximum is "always show the classification inline." The middle ground is "show only when days past window > some threshold" (e.g. > 30 days, on the assumption that most short delays are uninteresting). Worth discussing with users (well, with Javier) when v2 design solidifies.

---

## Out of scope for v2.0

To keep scope manageable, the v2.0 cycle should commit to **just `nog explain` and the integration flag**. Things to explicitly defer to v2.1+:

- Auto-resolution suggestions ("would you like nog to remove the spotify pin?") — too dangerous.
- AUR PKGBUILD freshness alerts ("aur/spotify hasn't been updated in 2 years") — feature creep.
- Suggesting `--ignore` flags for individual packages — leaks tier semantics into yay's surface.
- Integration with `nog search` to show why a package would be held — possible later, but not core.

---

## Open design questions for the v2 kickoff session

When the v2 cycle actually opens, these are the decisions that need a Javier+Claude session before any code:

1. Confirm the four-case taxonomy (A/B/C/D) is right. Add Case E if needed, prune anything unused.
2. Decide the AUR RPC caching strategy (TTL, on-disk location, opt-out).
3. Decide whether `nog update --diagnose` is opt-in or opt-out.
4. Decide on output format (human-default, JSON-on-flag, vs always-structured).
5. Decide on the testing approach for fuzzy classifiers (Cases B + C).
6. Decide whether to ship `nog explain --all-blocked` in v2.0 or defer.

---

## Cross-references

- **Vault session log:** `~/Google Drive/Rullynastre/Nog/2026-06-24.md` — captures the dogfood conversation that produced this design.
- **Vault catch-up:** `~/Google Drive/Rullynastre/Nog/2026-06-19.md` — current nog state summary.
- **Project memory:** `~/.claude/projects/-home-jetomev/memory/project_nog.md` — should be updated to reference this design doc.
- **Related parked work:** `project_deferred_external_distribution.md` — Arch Wiki page + Chaotic-AUR submission, parked pending external adoption signal. v2 ship would re-open that conversation.

---

*Design draft. No code. Re-evaluate at v2 cycle open.*
