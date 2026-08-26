# Test Matrix — nog v1.3.0 (#10: one package manager per source)

**Covers:** v1.2.1 (family coupling, #11) and v1.3.0 (handoff split, #10).
Both were built on 2026-08-25; v1.2.1 shipped that evening without a matrix, so
its checks are folded in here and marked **[carried]**.

**Machine:** tphome-linux (desktop Arch, KognogOS testing grounds)
**Binary under test:** `~/Programs/nog/target/debug/nog` unless stated
**Precondition for §2–§4:** at least one package Ready. On 2026-08-25 evening
everything was held, so the handoff was unreachable — see §0.

---

## §0 · Preconditions

| # | Check | Expected | Result |
|---|---|---|---|
| 0.1 | `checkupdates \| wc -l` | > 0 | |
| 0.2 | At least one official package past its window | Ready is non-empty | |
| 0.3 | At least one **AUR** package past its window | `fresh-editor-bin` or `snapd` Ready | |
| 0.4 | `nog --version` | `1.3.0` | |

> **0.3 is what makes §3 meaningful.** Without a cleared AUR package the helper
> step is skipped and never exercised. If it cannot be met, §3 must be recorded
> as **not run** rather than passed by inspection.

---

## §1 · Family coupling — #11 **[carried from v1.2.1]**

| # | Check | Expected | Result |
|---|---|---|---|
| 1.1 | pkgbase pair in one bucket | `elfutils`, `libelf`, `lib32-libelf` all Held | |
| 1.2 | Coupled rows name their partner | `coupled to lib32-libelf` | |
| 1.3 | Coupled rows share one countdown | identical days on all three | |
| 1.4 | Uniform cohort untouched | the ~67 nerd fonts all Held, none marked coupled | |
| 1.5 | Uniform cohort untouched | the ~34 vlc packages all Held, none marked coupled | |
| 1.6 | No over-firing | a Ready package with no held cohort sibling stays Ready | |
| 1.7 | Convergence | no "coupling did not converge" message ever printed | |

---

## §2 · Step 1 — pacman

| # | Check | Expected | Result |
|---|---|---|---|
| 2.1 | Step announced by name | `Handing off official packages to pacman ...` | |
| 2.2 | pacman runs, not the helper | no `yay` banner before the pacman transaction | |
| 2.3 | Held packages ignored | `warning: <pkg>: ignoring package upgrade` for held ones | |
| 2.4 | **No duplicate narration** | each held package announced **once**, not by both tools | |
| 2.5 | chaotic-aur handled here | any chaotic package upgrades in the pacman step | |
| 2.6 | Ready packages install | every Ready official package lands | |

> 2.4 is the headline of this release. Before the split, tonight's run printed
> ~140 `-> pkg: ignoring package upgrade` lines from yay **and** ~140
> `warning: pkg: ignoring package upgrade` lines from pacman — the same list
> twice. Count them.

---

## §3 · Step 2 — the AUR helper

| # | Check | Expected | Result |
|---|---|---|---|
| 3.1 | Step announced with a count | `Handing off N AUR package(s) to yay ...` | |
| 3.2 | Count matches the plan | N = Ready AUR + approved Unknown AUR | |
| 3.3 | **Only cleared names passed** | held AUR packages appear nowhere in yay's output | |
| 3.4 | No second sysupgrade | yay does **not** re-list official packages | |
| 3.5 | Cleared AUR package installs | the package actually upgrades | |
| 3.6 | Skipped when empty | with no cleared AUR package, yay is never invoked | |

---

## §4 · Steps 3–4 — flatpak and snap

| # | Check | Expected | Result |
|---|---|---|---|
| 4.1 | No flatpaks installed → silent | no flatpak step, no error | |
| 4.2 | No cleared snaps → silent | no snap step, no error | |
| 4.3 | Order preserved | pacman → helper → flatpak → snap | |

---

## §5 · Failure handling

Hard to trigger naturally; simulate where noted. **§5.1 is a release-blocker.**

| # | Check | Expected | Result |
|---|---|---|---|
| 5.1 | **pacman fails → cancel** | alert names the status; **no other source runs**; log says `pacman handoff failed (status N)` | |
| 5.2 | helper fails → ask | prompt `Continue with the remaining sources? [y/N]` | |
| 5.3 | Default is no | bare Enter stops the run | |
| 5.4 | EOF stops | `</dev/null` → `no input — stopping here.` | |
| 5.5 | Answering no | log says `cancelled after aur step failed (status N)` | |
| 5.6 | Answering yes | flatpak/snap still run; log says `installed with failures: ...` | |
| 5.7 | Summary printed | `Update finished, with N failed step(s).` + the list | |

> Simulation for 5.1: point `helper` at a non-existent binary, or run with a
> deliberately corrupt `--ignore` argument. Do **not** simulate by interrupting
> a real pacman transaction.

---

## §6 · Fence and holds still intact

| # | Check | Expected | Result |
|---|---|---|---|
| 6.1 | Fence message reworded | `held back as a dependency`, not `shielded from the handoff` | |
| 6.2 | Fence still populated | count matches uncleared foreign packages | |
| 6.3 | Held AUR package cannot move | absent from the helper's argument list | |
| 6.4 | AUR deactivated | `nog deactivate aur` → pacman step only, helper never invoked | |
| 6.5 | Reactivate | `nog activate aur` restores step 2 | |

---

## §7 · Report, log, and docs

| # | Check | Expected | Result |
|---|---|---|---|
| 7.1 | Per-source counts match execution | reported counts describe what actually ran | |
| 7.2 | CSV written | dated file under `~/.local/share/nog/logs/` | |
| 7.3 | Outcome column accurate | one of the documented values | |
| 7.4 | Man page matches behaviour | `man nog` four-step description is what happened | |
| 7.5 | README sample matches | the example output resembles the real run | |
| 7.6 | Version sync | binary, man page, README badge, PKGBUILD all `1.3.0` | |

---

## §8 · Regression slice from v1.2.0

| # | Check | Expected | Result |
|---|---|---|---|
| 8.1 | Unknown prompt still works | per-package `[y/N]` | |
| 8.2 | Kernel/headers desync warning | unchanged | |
| 8.3 | `--realign` | unchanged | |
| 8.4 | `nog search` tier colours | unchanged | |
| 8.5 | `nog install` | routes through the helper, unchanged | |
| 8.6 | All-held early exit | `Nothing to install — every pending update is held.` | |

---

## Not run, and why

*(fill in — publish this section even when it is long; a matrix that hides its
gaps is worse than no matrix)*

| Check | Why not run |
|---|---|
| | |
