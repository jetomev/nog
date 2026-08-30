//! Reboot advice (issue #9).
//!
//! Found live on 2026-08-10: a `nog update` installed new NVIDIA packages, DKMS
//! rebuilt the modules correctly, and the *old* module stayed loaded. The desktop
//! kept working until the first 3D application, which died with
//! `Failed to initialize NVML: Driver/library version mismatch`. Twenty minutes
//! went to suspecting the game. nog knew exactly what it had installed and said
//! nothing.
//!
//! The design rule, Javier 2026-08-30: **check the machine where nog can, and
//! name the package where it cannot** — never warn anonymously, and never claim
//! a finding that was not observed. So every line nog prints here is one of two
//! kinds, and says which it is:
//!
//! * [`Basis::Verified`] — nog compared the running system against what is now
//!   installed and they differ. This is a fact.
//! * [`Basis::Installed`] — nog confirmed the package was installed this run but
//!   has no way to ask what is running. This is advice.
//!
//! **nog recommends; nog never reboots anything.**

use std::collections::HashMap;

/// What a package's update actually calls for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Restart {
    /// The running kernel or a module loaded into it was replaced.
    Reboot,
    /// A display or session component was replaced; logging out is enough.
    Session,
}

/// Why nog is saying it — and the distinction is the whole point of the feature.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Basis {
    /// nog observed the difference. `running` and `installed` are what it saw.
    Verified { running: String, installed: String },
    /// nog confirmed the install but cannot inspect the running system.
    Installed,
}

/// One reason to restart, tied to the package that caused it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Advice {
    pub package: String,
    pub restart: Restart,
    pub basis: Basis,
    /// Plain-language consequence, shown only for verified findings.
    pub detail: String,
}

impl Advice {
    pub fn is_verified(&self) -> bool {
        matches!(self.basis, Basis::Verified { .. })
    }
}

/// Everything nog reads from the running system, gathered once **after** the
/// handoff so it reflects what pacman actually did rather than what nog asked
/// for. Kept as plain data so [`assess`] is pure and fully unit-testable.
#[derive(Debug, Default, Clone)]
pub struct SystemProbe {
    /// Does `/usr/lib/modules/<uname -r>/` still exist?
    ///
    /// This is the whole kernel check, and it deliberately parses no versions.
    /// The running kernel reports `7.0.5-zen1-1-zen` while its package reports
    /// `7.0.5.zen1-1`; comparing those two strings is a false-alarm generator.
    /// The modules directory is named for the running kernel and is removed when
    /// that kernel is replaced, so its absence *is* the finding.
    /// `None` means nog could not look.
    pub running_kernel_present: Option<bool>,
    /// The running kernel release, for reporting only — never for comparison.
    pub running_kernel: Option<String>,
    /// Driver version parsed out of `/proc/driver/nvidia/version`.
    /// `None` means no NVIDIA module is loaded, or the file was unreadable.
    pub nvidia_running: Option<String>,
    /// Version reported by `systemctl --version`.
    pub systemd_running: Option<String>,
    /// `pacman -Q` for the candidate packages, read after the handoff. A package
    /// missing here, or holding its old version, was not installed — the user
    /// can still decline individual packages at pacman's own prompt.
    pub installed_now: HashMap<String, String>,
}

/// Components where a new version is picked up by logging out, not by rebooting.
const SESSION_COMPONENTS: &[&str] = &["xorg-server", "wayland", "dbus", "mesa"];

/// Decide whether a package's update is worth mentioning, and what it calls for.
///
/// Tier 1 is the configured source of truth for "load-bearing", exactly as the
/// issue proposed, with three adjustments:
///
/// * `*-headers` are excluded. They are build inputs, never loaded into a
///   running system, and they are pulled in beside every kernel — including them
///   would put a second, meaningless line under every kernel update.
/// * NVIDIA and any `*-dkms` package are added. Tier 1 does not cover them, and
///   they are the case that produced this issue.
/// * The session components are separated out, because telling someone to reboot
///   when logging out is enough is the same kind of noise this feature exists to
///   avoid.
pub fn classify(package: &str, tier1: &[String]) -> Option<Restart> {
    if package.ends_with("-headers") {
        return None;
    }
    if SESSION_COMPONENTS.contains(&package) {
        return Some(Restart::Session);
    }
    if is_nvidia(package) || package.ends_with("-dkms") {
        return Some(Restart::Reboot);
    }
    if tier1.iter().any(|t| t == package) {
        return Some(Restart::Reboot);
    }
    None
}

/// `nvidia`, `nvidia-utils`, `nvidia-open-dkms`, `lib32-nvidia-utils`, …
pub fn is_nvidia(package: &str) -> bool {
    let base = package.strip_prefix("lib32-").unwrap_or(package);
    base == "nvidia" || base.starts_with("nvidia-")
}

/// Is this package one of the kernels — the thing the modules directory tracks?
pub fn is_kernel(package: &str, tier1: &[String]) -> bool {
    if package.ends_with("-headers") {
        return false;
    }
    tier1.iter().any(|t| t == package) && (package == "linux" || package.starts_with("linux-"))
}

/// Pull the driver version out of `/proc/driver/nvidia/version`.
///
/// The line reads, with irregular internal spacing:
/// `NVRM version: NVIDIA UNIX Open Kernel Module for x86_64  610.57.04  Release Build …`
/// so the version is found by shape — the first dotted-numeric token — rather
/// than by counting fields, which differ between the open and proprietary builds.
pub fn parse_nvidia_running(proc_text: &str) -> Option<String> {
    let line = proc_text.lines().find(|l| l.contains("NVRM version"))?;
    line.split_whitespace()
        .find(|tok| {
            tok.contains('.')
                && tok.chars().all(|c| c.is_ascii_digit() || c == '.')
                && tok.chars().next().is_some_and(|c| c.is_ascii_digit())
        })
        .map(|s| s.to_string())
}

/// Pull the version out of `systemctl --version`, whose first line reads
/// `systemd 261 (261.2-1-arch)`. The parenthesised token is the full package
/// version with a distribution suffix; the suffix is dropped so it can be
/// compared against what pacman reports.
pub fn parse_systemd_running(version_output: &str) -> Option<String> {
    let first = version_output.lines().next()?;
    let start = first.find('(')?;
    let end = first[start..].find(')')? + start;
    Some(strip_distro_suffix(&first[start + 1..end]))
}

/// `261.2-1-arch` → `261.2-1`. Drops trailing hyphen-separated segments that
/// carry no digits, which is what distribution suffixes look like; a real
/// `pkgrel` always does.
pub fn strip_distro_suffix(version: &str) -> String {
    let mut parts: Vec<&str> = version.split('-').collect();
    while parts.len() > 1
        && !parts
            .last()
            .is_some_and(|p| p.chars().any(|c| c.is_ascii_digit()))
    {
        parts.pop();
    }
    parts.join("-")
}

/// `610.57.04-1` → `610.57.04`. The running driver never reports a `pkgrel`, so
/// comparing against one would report a difference on every rebuild.
pub fn version_without_pkgrel(version: &str) -> &str {
    version.split('-').next().unwrap_or(version)
}

/// Work out what, if anything, to tell the user.
///
/// `cleared` is what nog handed off, as `(name, new_version)`. Membership is not
/// enough on its own: a package is only considered if [`SystemProbe::installed_now`]
/// confirms it actually reached the new version.
pub fn assess(
    cleared: &[(String, String)],
    tier1: &[String],
    probe: &SystemProbe,
) -> Vec<Advice> {
    let mut out: Vec<Advice> = Vec::new();

    for (name, new_version) in cleared {
        let Some(restart) = classify(name, tier1) else { continue };

        // Did it actually land? nog asked pacman for it; pacman answers to the
        // user, who may have declined this package at its own prompt.
        match probe.installed_now.get(name) {
            Some(now) if now == new_version => {}
            _ => continue,
        }

        // Kernels: the modules directory answers exactly, so a clean answer wins
        // over the announcement.
        if is_kernel(name, tier1) {
            match probe.running_kernel_present {
                Some(true) => continue, // still the running kernel; nothing stale
                Some(false) => {
                    out.push(Advice {
                        package: name.clone(),
                        restart: Restart::Reboot,
                        basis: Basis::Verified {
                            running: probe
                                .running_kernel
                                .clone()
                                .unwrap_or_else(|| "the running kernel".to_string()),
                            installed: new_version.clone(),
                        },
                        detail: "the kernel you are running is no longer installed"
                            .to_string(),
                    });
                    continue;
                }
                None => {} // fall through to the announcement
            }
        }

        // NVIDIA: the exact check that diagnosed the original incident.
        if is_nvidia(name) {
            if let Some(running) = &probe.nvidia_running {
                let installed = version_without_pkgrel(new_version);
                if running == installed {
                    continue; // module in memory already matches
                }
                out.push(Advice {
                    package: name.clone(),
                    restart: Restart::Reboot,
                    basis: Basis::Verified {
                        running: running.clone(),
                        installed: installed.to_string(),
                    },
                    detail: "3D applications will fail until you reboot".to_string(),
                });
                continue;
            }
            // No module loaded, or unreadable: cannot verify, so announce.
        }

        if name == "systemd" || name == "systemd-libs" {
            if let Some(running) = &probe.systemd_running {
                let installed = strip_distro_suffix(new_version);
                if running == &installed {
                    continue;
                }
                out.push(Advice {
                    package: name.clone(),
                    restart: Restart::Reboot,
                    basis: Basis::Verified {
                        running: running.clone(),
                        installed,
                    },
                    detail: "your machine is still running the old init system"
                        .to_string(),
                });
                continue;
            }
        }

        out.push(Advice {
            package: name.clone(),
            restart,
            basis: Basis::Installed,
            detail: String::new(),
        });
    }

    out
}

/// Render the advice as plain lines, newest concern first. Returns an empty
/// vector when there is nothing to say — silence is the common case and it must
/// stay cheap, because a notice that appears after every run is one nobody reads.
///
/// Colour is applied by the caller; keeping this plain keeps it testable.
pub fn render(items: &[Advice]) -> Vec<String> {
    if items.is_empty() {
        return Vec::new();
    }

    let mut lines: Vec<String> = Vec::new();
    let reboot: Vec<&Advice> = items.iter().filter(|a| a.restart == Restart::Reboot).collect();
    let session: Vec<&Advice> = items.iter().filter(|a| a.restart == Restart::Session).collect();

    if !reboot.is_empty() {
        lines.push("IMPORTANT: It is highly recommended to reboot the system!".to_string());
        for a in reboot.iter().filter(|a| a.is_verified()) {
            if let Basis::Verified { running, installed } = &a.basis {
                lines.push(format!(
                    "    verified — {}: running {}, installed {}.",
                    a.package, running, installed
                ));
                lines.push(format!("               {}.", a.detail));
            }
        }
        let announced: Vec<&str> = reboot
            .iter()
            .filter(|a| !a.is_verified())
            .map(|a| a.package.as_str())
            .collect();
        if !announced.is_empty() {
            lines.push(format!("    {}", updated_clause(&announced)));
            lines.push(
                "    nog cannot check these against the running system, so this is"
                    .to_string(),
            );
            lines.push("    advice, not a finding.".to_string());
        }
    }

    if !session.is_empty() {
        if !lines.is_empty() {
            lines.push(String::new());
        }
        let names: Vec<&str> = session.iter().map(|a| a.package.as_str()).collect();
        lines.push("NOTE: Log out and back in to pick up the new session components.".to_string());
        lines.push(format!("    {}", updated_clause(&names)));
    }

    lines
}

/// `glibc was updated this run.` / `glibc and mesa were updated this run.`
///
/// The verb is chosen with the list rather than after it — "glibc, mesa was
/// updated" reads as a defect in the tool, and this notice only ever appears at
/// a moment when the reader is already deciding whether to trust it.
fn updated_clause(names: &[&str]) -> String {
    let joined = match names {
        [] => return String::new(),
        [one] => return format!("{} was updated this run.", one),
        [a, b] => format!("{} and {}", a, b),
        [rest @ .., last] => format!("{} and {}", rest.join(", "), last),
    };
    format!("{} were updated this run.", joined)
}

impl SystemProbe {
    /// Read the running system. Call this **after** the handoff.
    ///
    /// Every probe is allowed to fail. A machine with no NVIDIA card, no
    /// `/proc` entry, or a `pacman` that will not answer must still finish the
    /// run normally — a missing probe downgrades a finding to advice, it never
    /// aborts anything and never prints an error of its own.
    pub fn read(candidates: &[String]) -> Self {
        let running_kernel = command_line("uname", &["-r"]);

        let running_kernel_present = running_kernel.as_ref().map(|rel| {
            std::path::Path::new(&format!("/usr/lib/modules/{}", rel)).is_dir()
        });

        let nvidia_running = std::fs::read_to_string("/proc/driver/nvidia/version")
            .ok()
            .and_then(|t| parse_nvidia_running(&t));

        let systemd_running =
            command_line("systemctl", &["--version"]).and_then(|o| parse_systemd_running(&o));

        SystemProbe {
            running_kernel_present,
            running_kernel,
            nvidia_running,
            systemd_running,
            installed_now: query_installed(candidates),
        }
    }
}

/// One `pacman -Q` for every candidate at once. Unknown names make pacman exit
/// non-zero while still printing the ones it does know, so the output is parsed
/// regardless of status — a package that is genuinely absent simply never
/// appears in the map, which is exactly how [`assess`] reads it.
fn query_installed(names: &[String]) -> HashMap<String, String> {
    let mut out = HashMap::new();
    if names.is_empty() {
        return out;
    }
    let Ok(o) = std::process::Command::new("pacman").arg("-Q").args(names).output() else {
        return out;
    };
    for line in String::from_utf8_lossy(&o.stdout).lines() {
        if let Some((name, version)) = line.split_once(' ') {
            out.insert(name.trim().to_string(), version.trim().to_string());
        }
    }
    out
}

/// Run a command and hand back its stdout, or `None` if it could not be run or
/// did not succeed.
fn command_line(program: &str, args: &[&str]) -> Option<String> {
    let o = std::process::Command::new(program).args(args).output().ok()?;
    if !o.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
    if s.is_empty() { None } else { Some(s) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tier1() -> Vec<String> {
        ["linux", "linux-zen", "systemd", "systemd-libs", "glibc", "mesa", "grub", "mkinitcpio"]
            .iter()
            .map(|s| s.to_string())
            .collect()
    }

    fn cleared(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs.iter().map(|(n, v)| (n.to_string(), v.to_string())).collect()
    }

    /// A probe that confirms every named package reached the given version.
    fn installed(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs.iter().map(|(n, v)| (n.to_string(), v.to_string())).collect()
    }

    // ---- classification -------------------------------------------------

    #[test]
    fn tier1_packages_call_for_a_reboot() {
        assert_eq!(classify("glibc", &tier1()), Some(Restart::Reboot));
        assert_eq!(classify("linux-zen", &tier1()), Some(Restart::Reboot));
    }

    #[test]
    fn headers_are_never_worth_mentioning() {
        // They ride along with every kernel and are never loaded into one.
        assert_eq!(classify("linux-headers", &tier1()), None);
        assert_eq!(classify("linux-zen-headers", &tier1()), None);
    }

    #[test]
    fn nvidia_and_dkms_are_reboot_even_though_tier1_omits_them() {
        assert_eq!(classify("nvidia-utils", &tier1()), Some(Restart::Reboot));
        assert_eq!(classify("lib32-nvidia-utils", &tier1()), Some(Restart::Reboot));
        assert_eq!(classify("nvidia-open-dkms", &tier1()), Some(Restart::Reboot));
        assert_eq!(classify("v4l2loopback-dkms", &tier1()), Some(Restart::Reboot));
    }

    #[test]
    fn session_components_do_not_ask_for_a_reboot() {
        assert_eq!(classify("xorg-server", &tier1()), Some(Restart::Session));
        assert_eq!(classify("wayland", &tier1()), Some(Restart::Session));
        assert_eq!(classify("dbus", &tier1()), Some(Restart::Session));
        // mesa is pinned Tier 1 but a logout is genuinely enough.
        assert_eq!(classify("mesa", &tier1()), Some(Restart::Session));
    }

    #[test]
    fn ordinary_packages_say_nothing() {
        assert_eq!(classify("firefox", &tier1()), None);
        assert_eq!(classify("libnvidia-container", &tier1()), None);
    }

    #[test]
    fn nvidia_matching_is_by_name_not_substring() {
        assert!(is_nvidia("nvidia"));
        assert!(is_nvidia("nvidia-utils"));
        assert!(is_nvidia("lib32-nvidia-utils"));
        assert!(!is_nvidia("libnvidia-container"));
        assert!(!is_nvidia("nvidiafoo"));
    }

    // ---- parsing --------------------------------------------------------

    #[test]
    fn nvidia_version_is_found_by_shape_in_the_open_module_banner() {
        let text = "NVRM version: NVIDIA UNIX Open Kernel Module for x86_64  610.57.04  \
                    Release Build  (root@)\nGCC version:  gcc version 16.1.1 20260728 (GCC)\n";
        assert_eq!(parse_nvidia_running(text), Some("610.57.04".to_string()));
    }

    #[test]
    fn nvidia_version_is_found_in_the_proprietary_banner_too() {
        let text = "NVRM version: NVIDIA UNIX x86_64 Kernel Module  550.120  \
                    Fri Sep 13 10:10:00 UTC 2026\n";
        assert_eq!(parse_nvidia_running(text), Some("550.120".to_string()));
    }

    #[test]
    fn nvidia_parse_refuses_text_it_does_not_recognise() {
        assert_eq!(parse_nvidia_running(""), None);
        assert_eq!(parse_nvidia_running("something else entirely\n"), None);
    }

    #[test]
    fn systemd_version_comes_from_the_parenthesised_token() {
        assert_eq!(
            parse_systemd_running("systemd 261 (261.2-1-arch)\n+PAM +AUDIT\n"),
            Some("261.2-1".to_string())
        );
    }

    #[test]
    fn systemd_parse_refuses_output_without_parentheses() {
        assert_eq!(parse_systemd_running("systemd 261\n"), None);
        assert_eq!(parse_systemd_running(""), None);
    }

    #[test]
    fn distro_suffix_is_dropped_but_pkgrel_is_kept() {
        assert_eq!(strip_distro_suffix("261.2-1-arch"), "261.2-1");
        assert_eq!(strip_distro_suffix("261.2-1"), "261.2-1");
        assert_eq!(strip_distro_suffix("1.0"), "1.0");
    }

    #[test]
    fn pkgrel_is_dropped_for_the_nvidia_comparison() {
        // The running driver never reports a pkgrel, so comparing against one
        // would report a difference on every rebuild.
        assert_eq!(version_without_pkgrel("610.57.04-1"), "610.57.04");
        assert_eq!(version_without_pkgrel("610.57.04"), "610.57.04");
    }

    // ---- the silences, which are the point ------------------------------

    #[test]
    fn a_kernel_update_that_left_the_running_kernel_installed_says_nothing() {
        // /usr/lib/modules/<running> still exists — nothing stale in memory.
        let probe = SystemProbe {
            running_kernel_present: Some(true),
            running_kernel: Some("7.0.5-zen1-1-zen".into()),
            installed_now: installed(&[("linux-zen", "7.0.6.zen1-1")]),
            ..Default::default()
        };
        assert!(assess(&cleared(&[("linux-zen", "7.0.6.zen1-1")]), &tier1(), &probe).is_empty());
    }

    #[test]
    fn nvidia_already_matching_in_memory_says_nothing() {
        let probe = SystemProbe {
            nvidia_running: Some("610.57.04".into()),
            installed_now: installed(&[("nvidia-utils", "610.57.04-1")]),
            ..Default::default()
        };
        assert!(assess(&cleared(&[("nvidia-utils", "610.57.04-1")]), &tier1(), &probe).is_empty());
    }

    #[test]
    fn a_package_the_user_declined_at_pacmans_prompt_says_nothing() {
        // nog cleared it, but pacman still reports the OLD version, so it was
        // never installed. Announcing here is the false alarm that would teach
        // the user to ignore the notice.
        let probe = SystemProbe {
            installed_now: installed(&[("glibc", "2.43-1")]), // old; nog asked for 2.44-1
            ..Default::default()
        };
        assert!(assess(&cleared(&[("glibc", "2.44-1")]), &tier1(), &probe).is_empty());
    }

    #[test]
    fn a_package_pacman_does_not_know_at_all_says_nothing() {
        let probe = SystemProbe::default(); // installed_now empty
        assert!(assess(&cleared(&[("glibc", "2.44-1")]), &tier1(), &probe).is_empty());
    }

    #[test]
    fn an_ordinary_run_says_nothing() {
        let probe = SystemProbe {
            installed_now: installed(&[("firefox", "141.0-1")]),
            ..Default::default()
        };
        assert!(assess(&cleared(&[("firefox", "141.0-1")]), &tier1(), &probe).is_empty());
        assert!(render(&[]).is_empty());
    }

    // ---- the findings ---------------------------------------------------

    #[test]
    fn a_replaced_running_kernel_is_reported_as_verified() {
        let probe = SystemProbe {
            running_kernel_present: Some(false),
            running_kernel: Some("7.0.5-zen1-1-zen".into()),
            installed_now: installed(&[("linux-zen", "7.0.6.zen1-1")]),
            ..Default::default()
        };
        let out = assess(&cleared(&[("linux-zen", "7.0.6.zen1-1")]), &tier1(), &probe);
        assert_eq!(out.len(), 1);
        assert!(out[0].is_verified());
        assert_eq!(out[0].restart, Restart::Reboot);
    }

    #[test]
    fn the_original_incident_is_reported_with_both_versions() {
        // 2026-08-10: nvidia-utils 610.43.03 -> 610.57.04, old module still loaded.
        let probe = SystemProbe {
            nvidia_running: Some("610.43.03".into()),
            installed_now: installed(&[("nvidia-utils", "610.57.04-1")]),
            ..Default::default()
        };
        let out = assess(&cleared(&[("nvidia-utils", "610.57.04-1")]), &tier1(), &probe);
        assert_eq!(out.len(), 1);
        assert_eq!(
            out[0].basis,
            Basis::Verified { running: "610.43.03".into(), installed: "610.57.04".into() }
        );
        let text = render(&out).join("\n");
        assert!(text.contains("610.43.03"), "the running version must be shown: {}", text);
        assert!(text.contains("610.57.04"), "the installed version must be shown: {}", text);
        assert!(text.contains("verified"));
    }

    #[test]
    fn systemd_running_behind_what_is_installed_is_verified() {
        let probe = SystemProbe {
            systemd_running: Some("260.4-1".into()),
            installed_now: installed(&[("systemd", "261.2-1")]),
            ..Default::default()
        };
        let out = assess(&cleared(&[("systemd", "261.2-1")]), &tier1(), &probe);
        assert_eq!(out.len(), 1);
        assert!(out[0].is_verified());
    }

    #[test]
    fn an_unprobeable_package_is_announced_not_claimed() {
        let probe = SystemProbe {
            installed_now: installed(&[("mkinitcpio", "40-1")]),
            ..Default::default()
        };
        let out = assess(&cleared(&[("mkinitcpio", "40-1")]), &tier1(), &probe);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].basis, Basis::Installed);
        let text = render(&out).join("\n");
        assert!(text.contains("advice, not a finding"), "must not claim a finding: {}", text);
    }

    #[test]
    fn nvidia_with_no_module_loaded_falls_back_to_announcing() {
        // nvidia_running is None — nog cannot see the running driver, so it must
        // not claim anything about it.
        let probe = SystemProbe {
            installed_now: installed(&[("nvidia-utils", "610.57.04-1")]),
            ..Default::default()
        };
        let out = assess(&cleared(&[("nvidia-utils", "610.57.04-1")]), &tier1(), &probe);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].basis, Basis::Installed);
    }

    #[test]
    fn a_kernel_that_could_not_be_probed_is_announced() {
        let probe = SystemProbe {
            running_kernel_present: None,
            installed_now: installed(&[("linux", "7.0.6-1")]),
            ..Default::default()
        };
        let out = assess(&cleared(&[("linux", "7.0.6-1")]), &tier1(), &probe);
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].basis, Basis::Installed);
    }

    // ---- the live half --------------------------------------------------

    /// Everything above drives [`assess`] with constructed data, which proves the
    /// logic and nothing about the probes. This one reads the real machine and is
    /// `#[ignore]`d because its result depends on the box it runs on — it is a
    /// diagnostic, not a regression test.
    ///
    /// Run it with:
    /// `cargo test --release live_probe -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn live_probe_sees_this_machine() {
        let probe = SystemProbe::read(&["pacman".to_string()]);
        println!("running kernel      : {:?}", probe.running_kernel);
        println!("its modules present : {:?}", probe.running_kernel_present);
        println!("nvidia running      : {:?}", probe.nvidia_running);
        println!("systemd running     : {:?}", probe.systemd_running);
        println!("pacman installed as : {:?}", probe.installed_now.get("pacman"));
        assert!(probe.running_kernel.is_some(), "uname -r should always answer");
    }

    /// Prints the exact text a user will see, for the incident that produced this
    /// issue plus an unverifiable package alongside it. `#[ignore]`d because it
    /// asserts nothing — it exists so the wording can be read and judged rather
    /// than inferred from format strings.
    ///
    /// `cargo test --release preview_the -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn preview_the_notice() {
        let probe = SystemProbe {
            nvidia_running: Some("610.43.03".into()),
            running_kernel_present: Some(false),
            running_kernel: Some("7.0.5-zen1-1-zen".into()),
            installed_now: installed(&[
                ("nvidia-utils", "610.57.04-1"),
                ("linux-zen", "7.0.6.zen1-1"),
                ("glibc", "2.44-1"),
                ("mkinitcpio", "40-1"),
                ("wayland", "1.25-1"),
            ]),
            ..Default::default()
        };
        let out = assess(
            &cleared(&[
                ("nvidia-utils", "610.57.04-1"),
                ("linux-zen", "7.0.6.zen1-1"),
                ("glibc", "2.44-1"),
                ("mkinitcpio", "40-1"),
                ("wayland", "1.25-1"),
            ]),
            &tier1(),
            &probe,
        );
        println!();
        for line in render(&out) {
            println!("{}", line);
        }
        println!();
    }

    // ---- rendering ------------------------------------------------------

    #[test]
    fn the_banner_is_the_wording_javier_asked_for() {
        let probe = SystemProbe {
            installed_now: installed(&[("glibc", "2.44-1")]),
            ..Default::default()
        };
        let out = assess(&cleared(&[("glibc", "2.44-1")]), &tier1(), &probe);
        assert_eq!(
            render(&out)[0],
            "IMPORTANT: It is highly recommended to reboot the system!"
        );
    }

    #[test]
    fn the_verb_agrees_with_the_number_of_packages() {
        let one = SystemProbe {
            installed_now: installed(&[("glibc", "2.44-1")]),
            ..Default::default()
        };
        let text = render(&assess(&cleared(&[("glibc", "2.44-1")]), &tier1(), &one)).join("\n");
        assert!(text.contains("glibc was updated"), "{}", text);

        let two = SystemProbe {
            installed_now: installed(&[("glibc", "2.44-1"), ("mkinitcpio", "40-1")]),
            ..Default::default()
        };
        let text = render(&assess(
            &cleared(&[("glibc", "2.44-1"), ("mkinitcpio", "40-1")]),
            &tier1(),
            &two,
        ))
        .join("\n");
        assert!(text.contains("glibc and mkinitcpio were updated"), "{}", text);
    }

    #[test]
    fn a_session_only_update_never_says_reboot() {
        let probe = SystemProbe {
            installed_now: installed(&[("xorg-server", "21.1.19-1")]),
            ..Default::default()
        };
        let out = assess(&cleared(&[("xorg-server", "21.1.19-1")]), &tier1(), &probe);
        let text = render(&out).join("\n");
        assert!(text.contains("Log out and back in"), "{}", text);
        assert!(!text.contains("reboot the system"), "must not demand a reboot: {}", text);
    }

    #[test]
    fn reboot_and_session_advice_can_appear_together_without_merging() {
        let probe = SystemProbe {
            installed_now: installed(&[("glibc", "2.44-1"), ("wayland", "1.25-1")]),
            ..Default::default()
        };
        let out = assess(&cleared(&[("glibc", "2.44-1"), ("wayland", "1.25-1")]), &tier1(), &probe);
        let text = render(&out).join("\n");
        assert!(text.contains("IMPORTANT:"));
        assert!(text.contains("NOTE:"));
        assert!(text.contains("glibc was updated"), "{}", text);
        assert!(text.contains("wayland was updated"), "{}", text);
    }
}
