# nog v1.8.0 — Test Results (4 Oct 2026)

Issue #47 (Javier: "why isn't nog doing the yay handoff (mention it) and then asking yes/no?"; option A).

| ID | What | Result |
|---|---|---|
| 1 | `cargo test --release --locked` | 242 passed, 5 ignored; warnings 6 |
| 2 | Claude, desktop, built program (answered n): `install cowsay`, `neofetch`, `aur/neofetch`, `pfetch-git`, `zzz-nope` | PASS — table each time; "unifetch provides it"; nog's question only for the AUR ones; not-found stops first |
| 3 | rc.1 built under a terminal (`script`), like yay | PASS — 242 tests in the build |
| 4 | Javier: `nog install` rc.1 package | PASS — only nog changed; the pacman handoff line even for a file |
| 5 | Javier: `nog install sl` | PASS — table (extra), pacman handoff, installed |
| 6 | Javier: `nog install neofetch` | PASS — the table said "chaotic-aur — unifetch provides it"; unifetch installed |
| 7 | Javier: `nog install zzzz` | PASS — stopped before anything: not found |
| 8 | Javier: `nog install aur/neofetch` | PASS — table (AUR — built by yay), "Install these? [Y/n]", yay handoff line, yay's menus and build (signature Passed), pacman's conflict question (unifetch removed); neofetch 7.1.0-2 in |

Still to come: Javier's install of 1.8.0 from the AUR.
