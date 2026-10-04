# nog v1.6.1 — Test Results (4 Oct 2026)

From Javier's test of nogForge's Update screen (nogForge matrix §8.4–8.6, `~/Programs/nogforge/testing/`).
Findings: **F-10 (#44)** a named list showed every pending update; **F-9 (#43)** a name found nowhere read as "you answered no".

## 1 · Claude, with the built program (scratch HOME, nothing installed)

| ID | Do | Expect | Result |
|---|---|---|---|
| 1.1 | `nog update git` (git held, 1 day) | stops before any question: "git is on hold (1 day remaining) and was not promoted. To bring it in now: nog update git --promote git"; exit 1 | PASS |
| 1.2 | `nog update vde2 freerdp`, answer n | one table UPDATING ONLY WHAT YOU NAMED with just those two; no SUMMARY, no held list, no fence note; "Cancelled — nothing was installed" | PASS |
| 1.3 | `nog update vde2 git zzznope --promote git` | stops on zzznope only ("no update waiting") | PASS |
| 1.4 | `nog install zzz-no-such-pkg` | "zzz-no-such-pkg was not found in the repositories or the AUR. Nothing was asked and nothing was changed." | PASS |
| 1.5 | name checks used by 1.4 | `base-devel` (group) and `git` found by pacman; `grubforge` found in the AUR; the made-up name in neither | PASS |
| 1.6 | `cargo test --release --locked` | 237 passed, 5 ignored; warnings 6 | PASS |

## 2 · Javier, on the desktop, through nogForge

| ID | Do | Result |
|---|---|---|
| 2.1 | `nog install` rc.1, then rc.2 (`dist-rc/`) | PASS — only nog changed each time (09:59, 10:12) |
| 2.2 | Update: freerdp unticked, u → `nog update vde2 wolfssl` | PASS — only those two shown and installed, exit 0 (10:04). pacman still printed ~70 "ignoring package upgrade" lines → Javier chose option (b): nog says first (rc.2) |
| 2.3 | git promoted, u → `nog update freerdp git --promote git` | PASS — only those two; "pacman will first list the 68 packages it is skipping"; pacman listed 68; git 2.56.0 and freerdp 3.32.1 in, exit 0 (10:33). Javier: *"exactly what was described!"* |

## Roll-up
9 PASS, 0 FAIL. Still to come: Javier's install of 1.6.1 from the AUR.
