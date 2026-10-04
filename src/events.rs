//! v1.7.0: nog's steps, for a program showing them (nogForge's steps view).
//!
//! When `NOG_EVENTS` names a file, nog appends one JSON line per event:
//! `{"ev":"steps","steps":[{"id","label"}]}` (what will run),
//! `{"ev":"step","id","label","state":"start|done|failed|skipped","detail"}`,
//! `{"ev":"ask","question"}` (nog's own question; pacman's are its own), and
//! `{"ev":"end","status"}`. Best-effort: a file that can't be written never
//! stops a run, and without `NOG_EVENTS` nothing happens at all. The words
//! are nog's own, so they don't change when pacman's wording does.

use serde_json::{json, Value};
use std::io::Write;

fn path() -> Option<String> {
    std::env::var("NOG_EVENTS").ok().filter(|p| !p.is_empty())
}

pub fn emit(v: &Value) {
    let Some(p) = path() else { return };
    if let Ok(mut f) = std::fs::OpenOptions::new().append(true).create(true).open(p) {
        let _ = writeln!(f, "{}", v);
    }
}

pub fn steps(list: &[(&str, String)]) {
    let s: Vec<Value> = list.iter().map(|(id, label)| json!({"id": id, "label": label})).collect();
    emit(&json!({"ev": "steps", "steps": s}));
}

pub fn step(id: &str, label: &str, state: &str, detail: &str) {
    emit(&json!({"ev": "step", "id": id, "label": label, "state": state, "detail": detail}));
}

pub fn start(id: &str, label: &str) {
    step(id, label, "start", "");
}

pub fn finished(id: &str, label: &str, ok: bool, detail: &str) {
    step(id, label, if ok { "done" } else { "failed" }, detail);
}

pub fn ask(question: &str) {
    emit(&json!({"ev": "ask", "question": question}));
}

pub fn end(status: i32) {
    emit(&json!({"ev": "end", "status": status}));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_go_to_the_named_file_as_json_lines() {
        let dir = std::env::temp_dir().join(format!("nog-events-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("ev");
        std::env::set_var("NOG_EVENTS", &f);   // the only test touching NOG_EVENTS
        steps(&[("pacman", "Official packages".to_string())]);
        start("pacman", "Official packages");
        finished("pacman", "Official packages", true, "2 packages");
        ask("Begin the handoff?");
        end(0);
        std::env::remove_var("NOG_EVENTS");
        let lines: Vec<Value> = std::fs::read_to_string(&f).unwrap().lines()
            .map(|l| serde_json::from_str(l).unwrap()).collect();
        assert_eq!(lines.len(), 5);
        assert_eq!(lines[0]["ev"], "steps");
        assert_eq!(lines[0]["steps"][0]["label"], "Official packages");
        assert_eq!(lines[2]["state"], "done");
        assert_eq!(lines[2]["detail"], "2 packages");
        assert_eq!(lines[3]["question"], "Begin the handoff?");
        assert_eq!(lines[4]["status"], 0);
        emit(&json!({"ev": "x"}));          // no NOG_EVENTS: nothing, no error
        assert_eq!(std::fs::read_to_string(&f).unwrap().lines().count(), 5);
        let _ = std::fs::remove_dir_all(dir);
    }
}
