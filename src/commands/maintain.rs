//! v2.0.0 (C5, #7): `nog clean` across every source. One report of what each
//! source leaves behind — pacman's old downloads (v1.5.5) and its orphans, the
//! AUR helper's build trees, Flatpak's unused runtimes, Snap's kept old
//! revisions — each category picked on its own, then each removed by the tool
//! that owns it, in its own voice. Report first, act on a yes; `--json` is the
//! report alone, for nogForge; `--only a,b` and `--yes` are for a program or a
//! person who already knows.

use super::*;
use crate::{flatpak, maint, snap, sources};
use serde_json::json;
use std::path::PathBuf;

/// The pacman-cache half of the plan, as v1.5.5 computed it.
struct CachePlan {
    rows: Vec<CleanRow>,
    to_remove: Vec<PathBuf>,
    stale: Vec<PathBuf>,
    total_files: usize,
    total_bytes: u64,
    kept_files: usize,
    kept_bytes: u64,
}

fn cache_plan(cfg: &NogConfig, tm: &TierManager, installed: &HashMap<String, String>) -> CachePlan {
    let keep = |t: u8| -> usize {
        (match t { 1 => cfg.clean.tier1_keep, 2 => cfg.clean.tier2_keep, _ => cfg.clean.tier3_keep }) as usize
    };
    let mut p = CachePlan { rows: Vec::new(), to_remove: Vec::new(), stale: Vec::new(), total_files: 0, total_bytes: 0, kept_files: 0, kept_bytes: 0 };
    for dir in cache::cache_dirs(&cfg.paths.pacman_conf) {
        let listing = cache::list_dir(&dir);
        if listing.files.is_empty() && listing.stale_dirs.is_empty() { continue; }
        p.total_files += listing.files.len();
        p.total_bytes += listing.files.iter().map(|(_, b)| b).sum::<u64>();
        let plan = cache::plan(cache::read_listing(&listing.files), installed, |n| tier_num(&tm.classify(n)), keep);
        for (f, reason) in &plan.remove {
            let label = match reason {
                cache::Reason::Older(t) => format!("Tier {} · older than the newest {}", t, keep(*t).max(1)),
                cache::Reason::NotInstalled => "No longer installed".to_string(),
            };
            match p.rows.iter_mut().find(|r| r.what == label) {
                Some(r) => { r.versions += 1; r.bytes += cache::Plan::bytes(f); }
                None => p.rows.push(CleanRow { what: label, versions: 1, bytes: cache::Plan::bytes(f) }),
            }
            p.to_remove.push(dir.join(&f.file));
            if let Some((sig, _)) = &f.sig { p.to_remove.push(dir.join(sig)); }
        }
        for f in &plan.keep { p.kept_files += 1; p.kept_bytes += cache::Plan::bytes(f); }
        p.stale.extend(listing.stale_dirs.iter().map(|d| dir.join(d)));
    }
    p.rows.sort_by(|a, b| a.what.cmp(&b.what));
    p
}

/// The whole report: what there is to clean, per source.
struct Report {
    cache: CachePlan,
    categories: Vec<maint::Category>,
    helper_dir: Option<PathBuf>,
    snap_revisions: Vec<(String, String)>,
}

fn build_report(cfg: &NogConfig, tm: &TierManager) -> Option<Report> {
    let installed = pacman::all_installed_versions()?;
    let cache = cache_plan(cfg, tm, &installed);
    let mut categories: Vec<maint::Category> = Vec::new();
    let cache_versions: usize = cache.rows.iter().map(|r| r.versions).sum::<usize>();
    categories.push(maint::Category {
        id: "cache",
        what: format!("pacman's download cache: {} old version{} of packages{}", cache_versions, if cache_versions == 1 { "" } else { "s" },
            if cache.stale.is_empty() { String::new() } else { format!(" and {} leftover download folder{}", cache.stale.len(), if cache.stale.len() == 1 { "" } else { "s" }) }),
        items: cache_versions + cache.stale.len(),
        bytes: cache.rows.iter().map(|r| r.bytes).sum(),
        names: Vec::new(),
        how: "removed by nog; root is needed".to_string(),
    });
    let local = crate::machine::load_installed(std::path::Path::new("/var/lib/pacman/local"));
    let size_of = |n: &str| local.iter().find(|p| p.name == n).map(|p| p.size).unwrap_or(0);
    let orphans = maint::orphans(&size_of);
    categories.push(maint::Category {
        id: "orphans",
        what: format!("packages installed as dependencies that nothing needs any more: {}", orphans.len()),
        items: orphans.len(),
        bytes: orphans.iter().map(|o| o.size).sum(),
        names: orphans.iter().map(|o| o.name.clone()).collect(),
        how: "pacman -Rns: pacman shows the list and asks first".to_string(),
    });
    let helper = resolve_helper(cfg);
    let helper_dir = helper.map(|h| maint::helper_cache_dir(h.binary(), std::path::Path::new(&std::env::var("HOME").unwrap_or_default())));
    if let (Some(h), Some(dir)) = (helper, &helper_dir) {
        let trees = maint::build_trees(dir);
        categories.push(maint::Category {
            id: "aur",
            what: format!("{}'s build trees from AUR packages: {} (in {})", h, trees.len(), dir.display()),
            items: trees.len(),
            bytes: trees.iter().map(|t| t.bytes).sum(),
            names: trees.iter().map(|t| t.name.clone()).collect(),
            how: "removed by nog; your own files, so no password".to_string(),
        });
    }
    let state = sources::load(sources::DEFAULT_PATH);
    if state.flatpak && flatpak::is_available() {
        let unused = flatpak::unused_runtimes(&flatpak::installed_detail(), &flatpak::installed_runtimes(), flatpak::host_nvidia_driver().as_deref());
        categories.push(maint::Category {
            id: "flatpak",
            what: format!("Flatpak runtimes no installed app uses: {} (nog's estimate; flatpak decides the exact list)", unused.len()),
            items: unused.len(),
            bytes: unused.iter().map(|r| r.size).sum(),
            names: unused.iter().map(|r| r.reference.clone()).collect(),
            how: "flatpak uninstall --unused: flatpak works out the exact list, shows it and removes it (your pick here is the yes)".to_string(),
        });
    }
    let mut snap_revisions = Vec::new();
    if state.snap && snap::is_available() {
        let old: Vec<snap::Installed> = snap::list_all().into_iter().filter(|s| s.disabled()).collect();
        snap_revisions = old.iter().map(|s| (s.name.clone(), s.revision.clone())).collect();
        categories.push(maint::Category {
            id: "snap",
            what: format!("old snap revisions snapd kept after updates: {}", old.len()),
            items: old.len(),
            bytes: old.iter().map(|s| snap::revision_size(&s.name, &s.revision)).sum(),
            names: old.iter().map(|s| format!("{} (revision {})", s.name, s.revision)).collect(),
            how: "snap remove --revision, one by one; root is needed".to_string(),
        });
    }
    Some(Report { cache, categories, helper_dir, snap_revisions })
}

fn report_json(r: &Report) -> serde_json::Value {
    json!({
        "nog": env!("CARGO_PKG_VERSION"), "kind": "clean",
        "categories": r.categories.iter().map(|c| json!({
            "id": c.id, "what": c.what, "items": c.items, "bytes": c.bytes, "names": c.names, "how": c.how,
        })).collect::<Vec<_>>(),
        "total_bytes": r.categories.iter().map(|c| c.bytes).sum::<u64>(),
        "cache": {"files": r.cache.total_files, "bytes": r.cache.total_bytes, "kept_files": r.cache.kept_files, "kept_bytes": r.cache.kept_bytes},
    })
}

/// Pure. The report as a person reads it: a numbered row per category.
pub(super) fn format_report(cats: &[maint::Category]) -> String {
    let w_what = cats.iter().map(|c| c.what.chars().count()).max().unwrap_or(4).max(4);
    let w_size = cats.iter().map(|c| human_size(c.bytes).len()).max().unwrap_or(4).max(4);
    let mut out = format!("    {:>2}  {:<ww$}  {:>ws$}\n", "#", "What", "Size", ww = w_what, ws = w_size);
    out.push_str(&format!("    {}\n", "-".repeat(w_what + w_size + 6)));
    for (i, c) in cats.iter().enumerate() {
        let pad = w_what - c.what.chars().count();
        out.push_str(&format!("    {:>2}  {}{}  {:>ws$}\n", i + 1, c.what, " ".repeat(pad), if c.items == 0 { "—".to_string() } else { human_size(c.bytes) }, ws = w_size));
    }
    let total: u64 = cats.iter().map(|c| c.bytes).sum();
    out.push_str(&format!("    {}\n", "-".repeat(w_what + w_size + 6)));
    out.push_str(&format!("    {:>2}  {:<ww$}  {:>ws$}\n", "", "Total", human_size(total), ww = w_what, ws = w_size));
    out
}

/// `nog clean [--json] [--yes] [--only a,b]`.
pub fn clean(json: bool, yes: bool, only: &[String]) {
    let cfg = load_config();
    let tm = load_tiers();
    let Some(report) = build_report(&cfg, &tm) else {
        enotice(C_PEACH, "Not cleaning", &["pacman could not say what is installed — stopping, nothing removed.".to_string(),
            "(Without that list every cached file would look unused.)".to_string()]);
        end(1);
    };
    if json {
        crate::machine::emit(&report_json(&report));
        return;
    }
    let ids: Vec<&str> = report.categories.iter().map(|c| c.id).collect();
    let picked_ids: Vec<String> = match maint::pick_only(only, &ids) {
        Ok(p) => p,
        Err(bad) => {
            enotice(C_PEACH, "Not cleaning", &[format!("--only names {} which {} not a category here. The categories: {}.",
                bad.join(", "), if bad.len() == 1 { "is" } else { "are" }, ids.join(", ")), "Nothing was removed.".to_string()]);
            end(1);
        }
    };
    let cats = &report.categories;
    notice(C_GREEN, "What each source has left behind", &[format!(
        "pacman's cache holds {} files, {}; {} package versions stay, including every installed one.",
        report.cache.total_files, human_size(report.cache.total_bytes), report.cache.kept_files)]);
    print!("{}", format_report(cats));
    for c in cats.iter().filter(|c| c.items > 0 && !c.names.is_empty()) {
        let shown: Vec<&str> = c.names.iter().take(12).map(String::as_str).collect();
        println!("    {}{}: {}{}{}", C_SUBTEXT, c.id, shown.join(", "), if c.names.len() > 12 { format!(", … {} more", c.names.len() - 12) } else { String::new() }, C_RESET);
    }
    if !report.cache.rows.is_empty() {
        println!();
        for l in format_clean_report(&report.cache.rows, report.cache.stale.len()).lines().skip(3) {
            println!("    {}{}{}", C_SUBTEXT, l, C_RESET);
        }
    }
    println!();
    for c in cats { println!("    {}{} — {}{}", C_SUBTEXT, c.id, c.how, C_RESET); }
    let with_items: Vec<usize> = cats.iter().enumerate().filter(|(_, c)| c.items > 0).map(|(i, _)| i).collect();
    if with_items.is_empty() {
        println!();
        println!("nog: Nothing to clean.");
        return;
    }
    let chosen: Vec<usize> = if !picked_ids.is_empty() {
        let picked: Vec<usize> = cats.iter().enumerate().filter(|(_, c)| picked_ids.iter().any(|p| p == c.id) && c.items > 0).map(|(i, _)| i).collect();
        if picked.is_empty() {
            println!();
            println!("nog: Nothing to clean in {}.", picked_ids.join(", "));
            return;
        }
        // --only picks; it does not answer. The question stays unless --yes.
        if !yes {
            if !at_the_keyboard() {
                enotice(C_PEACH, "Not cleaning", &["Nothing is at the keyboard to confirm, and nothing is removed without that.".to_string(),
                    format!("Run this in a terminal, or add --yes: nog clean --only {} --yes", picked_ids.join(","))]);
                end(1);
            }
            let names: Vec<&str> = picked.iter().map(|i| cats[*i].id).collect();
            if !prompt_yes_no(&format!("nog: Clean up {}? [y/N] ", names.join(", ")), false) {
                println!();
                println!("nog: Nothing removed.");
                return;
            }
        }
        picked
    } else if yes {
        with_items.clone()
    } else {
        if !at_the_keyboard() {
            enotice(C_PEACH, "Not cleaning", &["Nothing is at the keyboard to pick what to clean, and nothing is removed without that.".to_string(),
                "Run this in a terminal, or say what to clean: nog clean --only cache,orphans (--yes answers the tools' questions too).".to_string()]);
            end(1);
        }
        let question = format!("nog: Clean up which? [all, numbers like 1 3, or n for none] ");
        let mut picked = None;
        for _ in 0..3 {
            crate::events::ask(question.trim_start_matches("nog: ").trim());
            print!("\n{}", question);
            use std::io::Write;
            let _ = std::io::stdout().flush();
            let mut buf = String::new();
            if !matches!(std::io::stdin().read_line(&mut buf), Ok(n) if n > 0) { break; }
            match maint::parse_choice(&buf, cats.len()) {
                Some(p) => { picked = Some(p); break; }
                None => println!("nog: a number from 1 to {}, several of them, all, or n.", cats.len()),
            }
        }
        picked.unwrap_or_default().into_iter().filter(|i| cats[*i].items > 0).collect()
    };
    if chosen.is_empty() {
        println!();
        println!("nog: Nothing removed.");
        return;
    }
    if chosen.iter().any(|i| matches!(cats[*i].id, "cache" | "orphans")) && cache::pacman_is_running() {
        enotice(C_PEACH, "Not cleaning", &["pacman is running (its lock file exists) — stopping, nothing removed.".to_string()]);
        end(1);
    }
    let steps: Vec<(&str, String)> = chosen.iter().map(|i| (cats[*i].id, format!("Cleaning: {}", cats[*i].id))).collect();
    crate::events::steps(&steps);
    let mut failed = false;
    let mut kept: Vec<&str> = Vec::new();
    let before: u64 = chosen.iter().map(|i| cats[*i].bytes).sum();
    let n_chosen = chosen.len();
    for i in chosen {
        let c = &cats[i];
        let label = format!("Cleaning: {}", c.id);
        crate::events::start(c.id, &label);
        let ok = match c.id {
            "cache" => {
                notice(C_GREEN, "Removing old downloads", &[format!("{} from pacman's cache; root is needed.", human_size(c.bytes))]);
                let mut ok = true;
                for batch in report.cache.to_remove.chunks(400) {
                    ok &= matches!(crate::machine::sudo().args(["rm", "-f", "--"]).args(batch).status(), Ok(s) if s.success());
                }
                for batch in report.cache.stale.chunks(100) {
                    ok &= matches!(crate::machine::sudo().args(["rm", "-rf", "--"]).args(batch).status(), Ok(s) if s.success());
                }
                ok
            }
            "orphans" => {
                notice(C_GREEN, &format!("Removing {} orphan{}", c.items, if c.items == 1 { "" } else { "s" }), &[
                    if yes { "pacman shows the list and, with your yes (--yes), removes them without asking.".to_string() }
                    else { "pacman shows the list and asks first; n keeps them.".to_string() }]);
                let st = pacman::remove(&c.names, yes);
                if st.success() { true } else {
                    // F-19: pacman exits 1 both for "n" at its question and for a
                    // real error. Count what is left: all still there = the person
                    // kept them, which is a choice, not a failure.
                    let left = maint::orphans(&|_| 0).len();
                    if left >= c.items { kept.push(c.id); true } else { false }
                }
            }
            "aur" => {
                let dir = report.helper_dir.clone().unwrap_or_default();
                notice(C_PEACH, "Removing the helper's build trees", &[format!("{} folders under {}; yours, so no password. The helper clones again when it needs them.", c.items, dir.display())]);
                let mut ok = true;
                for n in &c.names {
                    let p = dir.join(n);
                    if p.starts_with(&dir) && p != dir {
                        ok &= std::fs::remove_dir_all(&p).is_ok();
                    }
                }
                ok
            }
            "flatpak" => {
                notice(C_BLUE, "Removing unused Flatpak runtimes", &[
                    if yes { "flatpak works out the exact list and, with your yes (--yes), removes it without asking.".to_string() }
                    // F-18: flatpak 1.18 does not ask for --unused; the pick at nog's question was the yes.
                    else { "flatpak works out the exact list, shows it and removes it; your pick above was the yes.".to_string() }]);
                flatpak::uninstall_unused(yes).status.success()
            }
            "snap" => {
                notice(C_MAUVE, "Removing old snap revisions", &[format!("{} kept revision{}; `snap remove --revision` for each, root is needed. The active revisions stay.", c.items, if c.items == 1 { "" } else { "s" })]);
                let mut ok = true;
                for (name, rev) in &report.snap_revisions {
                    ok &= snap::remove_revision(name, rev).status.success();
                }
                ok
            }
            _ => true,
        };
        let was_kept = kept.last() == Some(&c.id);
        crate::events::finished(c.id, &label, ok, if was_kept { "kept" } else { "" });
        if was_kept {
            println!();
            println!("{}nog: {} kept — you answered n at pacman's question.{}", C_SUBTEXT, c.id, C_RESET);
        }
        if !ok {
            failed = true;
            eprintln!();
            eprintln!("{}nog: {} did not finish cleanly (see above); going on with the rest.{}", C_PEACH, c.id, C_RESET);
        }
    }
    println!();
    if kept.len() == n_chosen {
        // F-19: everything picked was kept at the tool's question — nothing went, nothing failed.
        println!("nog: Nothing removed.");
        return;
    }
    let mut lines = vec![format!("About {} was in the categories you picked (the tools' own output above is what really went).", human_size(before))];
    if !kept.is_empty() { lines.push(format!("Kept at your n: {}.", kept.join(", "))); }
    notice(if failed { C_PEACH } else { C_GREEN }, if failed { "Cleaned, with problems" } else { "Cleaned" }, &lines);
    if failed { end(1); }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_report_is_a_numbered_table_with_a_total_and_dashes_for_empty_rows() {
        let cats = vec![
            maint::Category { id: "cache", what: "pacman's download cache: 3 old versions of packages".into(), items: 3, bytes: 3 * 1024 * 1024, names: vec![], how: String::new() },
            maint::Category { id: "orphans", what: "packages nothing needs any more: 0".into(), items: 0, bytes: 0, names: vec![], how: String::new() },
        ];
        let t = format_report(&cats);
        assert!(t.contains(" 1  pacman's download cache"), "{}", t);
        assert!(t.contains(" 2  packages nothing needs any more: 0"), "{}", t);
        assert!(t.lines().nth(3).unwrap().trim_end().ends_with('—'), "an empty row shows a dash, not 0 B: {}", t);
        let last = t.trim_end().lines().last().unwrap();
        assert!(last.contains("Total") && last.ends_with("3 MB"), "{}", t);
    }
}
