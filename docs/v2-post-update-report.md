# nog v2 — post-update report (`nog`'s equivalent of apt's advisory)

**Status:** DESIGN DRAFT — captured 2026-06-30. Not yet a committed v2.0 feature; no code written. Sibling to [`v2-explain.md`](v2-explain.md) — both are candidate v2 features.

**Triggered by:** Javier's request (2026-06-30) — *"at the end of the update, present a summary of what type and how many files of each type were updated, and whether a reboot would be needed based on what was updated. Something like apt does. Any important information a user should know about their update."*

---

## The thesis

Right now `nog update` hands off to yay/pacman and the transaction ends with pacman's raw output — a wall of `(x/y) upgrading <pkg>` lines and post-transaction hooks. The user is left to *infer* what actually happened and what they should do next.

Debian/Ubuntu's apt ecosystem does better: `needrestart` tells you which services are running outdated libraries, `/var/run/reboot-required` flags kernel/libc updates, and the tooling surfaces "you should reboot" or "these services need restarting." Arch has the *building blocks* (uname vs installed kernel, needrestart-in-AUR, pacman hooks) but nothing that ties it into a clean end-of-update summary.

**`nog` is uniquely positioned to do this well** because it already wraps the update transaction — it knows exactly which packages were updated, at which tiers, and it already has opinions about package criticality (the tier system). Adding a **post-update report** turns the end of every `nog update` into a clear, categorized, actionable summary.

This complements the tier system beautifully: tiers *delay* risky updates; the post-update report gives *awareness* of what an update actually changed and what to do about it.

---

## What the report should show

Three sections, printed after the transaction completes:

### 1. Categorized summary — "what type and how many"

Classify the updated packages into functional categories and count them:

```
nog: update complete — 145 packages updated

  Kernel & boot        3    (linux-zen, linux-zen-headers, mkinitcpio)
  Firmware & microcode 12   (linux-firmware-*, amd-ucode)
  Core system          4    (systemd, systemd-libs, glibc, util-linux)
  Graphics stack       6    (mesa, vulkan-icd-loader, libdrm, ...)
  Desktop (KDE/Plasma) 51   (plasma-*, kf6-*, kwin, ...)
  Audio                8    (pipewire-*, wireplumber, ...)
  Applications         38   (vlc, thunderbird, obsidian, ...)
  Libraries            19   (fmt, libheif, x265, ...)
  Fonts / themes / icons  2
  Development          2    (python, dotnet-runtime)
```

The categories are derived from package-name patterns + pacman group membership + a small curated rule set. Anything unmatched falls into "Applications" or "Libraries" based on whether it provides a `.so`.

### 2. Reboot / restart advisory — "should I reboot?"

The headline feature. Based on *what* was in the update set, nog computes and prints a clear verdict:

```
  ⚠ REBOOT RECOMMENDED
    • Kernel updated: running 7.0.5-zen1, installed 7.0.12-zen1
      → reboot to run the new kernel (modules for the old one may be gone)
    • Microcode updated (amd-ucode) → reboot to load new CPU microcode
    • systemd updated → 'systemctl daemon-reexec' done by hook, but a
      reboot is cleaner for a version bump
```

or, when nothing critical changed:

```
  ✓ No reboot needed — only applications and libraries were updated.
    (Optional: restart any long-running apps that were upgraded.)
```

**Reboot decision logic (tiered by severity):**

| Trigger | Verdict |
|---|---|
| Kernel package updated **AND** `uname -r` ≠ installed kernel version | **REBOOT REQUIRED** (running kernel's modules may be gone) |
| Microcode (`amd-ucode`/`intel-ucode`) or `linux-firmware` updated | **REBOOT RECOMMENDED** (to load new firmware/microcode) |
| `systemd` / `systemd-libs` version bump | **REBOOT RECOMMENDED** (daemon-reexec helps but reboot is clean) |
| `glibc` updated | **REBOOT RECOMMENDED** (core lib; nearly everything links it) |
| Graphics driver (`nvidia*`, `mesa`) or Xorg/Wayland core updated | **SESSION RESTART RECOMMENDED** (log out/in, or reboot, for the display stack) |
| Desktop environment major bump (e.g. Plasma 6.6→6.7) | **SESSION RESTART RECOMMENDED** |
| Only apps/leaf-libraries updated | **NO REBOOT** — optionally restart the specific apps |

The kernel check is the important one and is deterministic: compare `uname -r` against the version of the installed `linux*` package that matches the running kernel flavour. If they differ after an update, the running kernel is now "orphaned" from its modules → reboot.

### 3. Service restart advisory (needrestart-style) — optional, deeper

Detect **running services/processes still using outdated (deleted) shared libraries** — the thing `needrestart` does on Debian. After a library update, a long-running daemon keeps the *old* library mapped (visible as `(deleted)` in `/proc/<pid>/maps`). Those services should be restarted to pick up the fix (especially security-relevant, e.g. openssl/openssh).

```
  ⟳ SERVICES USING OUTDATED LIBRARIES (consider restarting):
    • sshd        (using deleted libcrypto)   → sudo systemctl restart sshd
    • pipewire    (using deleted libpipewire) → restarts with session
```

This is the most valuable-but-complex part. Could be v2.1 if v2.0 ships just sections 1+2.

### 4. Important notices — ".pacnew, .pacsave, and anything else"

Surface things pacman hooks emit but users miss in the scroll:

```
  📋 NOTICES
    • 2 new .pacnew config files — review + merge:
        /etc/pacman.conf.pacnew
        /etc/ssh/sshd_config.pacnew
    • x265 major version 4.1 → 4.2 (SOVERSION change; ffmpeg was rebuilt)
```

`.pacnew`/`.pacsave` detection is cheap (scan `/etc` for files newer than the transaction). This is genuinely useful — un-merged `.pacnew` files are a classic Arch footgun.

---

## Data sources needed

| Source | Purpose | Cost |
|---|---|---|
| The update package list (nog already has it) | Categorization + reboot triggers | Free — nog owns the transaction |
| `uname -r` | Running kernel version for the kernel check | Cheap |
| `pacman -Q linux*` | Installed kernel version(s) | Cheap — local DB |
| Package-name patterns + `pacman -Qg` groups | Category classification | Cheap |
| `/proc/<pid>/maps` scan for `(deleted)` libs | needrestart-style service detection (section 3) | Medium — scan running procs |
| Filesystem scan of `/etc` for fresh `.pacnew`/`.pacsave` | Notices (section 4) | Cheap |
| `/var/log/pacman.log` | Cross-check what the transaction actually did | Cheap |

Sections 1, 2, 4 are all cheap and offline. Section 3 (needrestart) is the only one needing process scanning — reasonable to gate behind a flag or defer to v2.1.

---

## Command surface

- **Default:** the report prints automatically at the end of every `nog update` (the whole point — apt-like advisory without asking).
- **`nog update --no-report`:** suppress it (for scripting).
- **`nog report`:** re-run the report against the *last* transaction (reads `/var/log/pacman.log` for the most recent upgrade set) — useful if the user scrolled past it or wants to re-check "do I still need to reboot?"
- **`nog report --json`:** machine-readable, for status bars / dashboards (ties into the fish `sysinfo.py` greeting — could show "⚠ reboot pending" in the prompt).

The `--json` + `nog report` combo is a nice hook: the KognogOS fish greeting (`sysinfo.py`) could surface "reboot recommended since last update" the same way it surfaces tier notifications today.

---

## Why this fits nog specifically

1. **nog already owns the transaction** — it knows the exact package set, no guessing.
2. **The tier system already classifies criticality** — kernel/systemd/mesa are Tier 1 for exactly the reasons that make them reboot-relevant. The reboot logic and the tier logic draw on the same "these packages are load-bearing" intuition.
3. **Arch has no native reboot-required mechanism** — this is a real gap nog can fill cleanly, and it's the #1 thing new Arch users don't know ("do I need to reboot after this?").
4. **It's honest, calm, and informative** — matches nog's whole ethos (the tier system is about giving users control + awareness, not magic).

---

## Scope split for v2.0

To keep it shippable, v2.0 could commit to **sections 1, 2, and 4** (categorized summary + reboot advisory + .pacnew notices) — all cheap, offline, deterministic. Defer **section 3** (needrestart-style running-library detection) to v2.1, since it needs process scanning and is the fiddliest to get right.

Minimum-viable v2.0 post-update report = "here's what categories updated, here's whether you should reboot and why, here are your .pacnew files." That alone is a big UX win over raw pacman output.

---

## Open questions for the v2 kickoff

1. How granular should categories be? (The list above is ~10 — could be fewer.) Curated rule set vs. pacman groups vs. hybrid?
2. Reboot verdict thresholds — is `glibc` "recommended" or just "informational"? How aggressive?
3. Should the report always print, or only when there's something noteworthy (reboot/notices)? Probably always for the summary, but keep it terse when nothing's critical.
4. Section 3 (needrestart) in v2.0 or v2.1?
5. Does `nog report --json` feed the KognogOS greeting? (Nice integration, worth designing the JSON shape with that consumer in mind.)
6. Interaction with the existing tier-notification output — unify the visual language.

---

## Cross-references

- Sibling v2 feature: [`v2-explain.md`](v2-explain.md) — the `nog explain <pkg>` diagnostic command.
- Both are candidate v2.0 features; the v2 kickoff session decides scope (one, both, or phased).
- Project memory: `~/.claude/projects/-home-jetomev/memory/project_nog.md`.
- KognogOS greeting integration: `~/Programs/kognog/config/sysinfo.py` (already surfaces tier notifications; could surface "reboot pending").

---

*Design draft. No code. Re-evaluate at v2 cycle open. Requested by Javier 2026-06-30.*
