//! Run a handoff and remember why it failed (v1.4.3, issue #21).
//!
//! Until now every source step ran with `.status()`: the tool's output went
//! straight to the terminal and nog kept only the exit code. A `status 1`
//! from pacman covers a download failure, a dependency conflict, a bad
//! signature and a user answering "n" alike, and once the terminal closed
//! there was no way back to which one it was.
//!
//! stderr is now passed through nog on its way to the terminal. Every byte is
//! forwarded the moment it arrives — not line by line — because pacman writes
//! its questions to stderr with no trailing newline (`:: Proceed with
//! installation? [Y/n] `) and a line-buffered relay would hide the prompt the
//! user is being asked to answer. Issue #8's rule stands: the handoff's own
//! output stays live and visible. nog only keeps the last few kilobytes.
//!
//! stdout is untouched, so progress bars, which pacman draws only when stdout
//! is a terminal, behave exactly as before.
//!
//! v1.5.3 (F-1 of v1.5.2): when stderr is a terminal, every bare line feed is
//! relayed as CR LF. sudo 1.9.14+ runs the command in its own pseudo-terminal
//! (`use_pty`, on by default) and switches the user's terminal to raw mode
//! while it runs. stdout passes through that pty and gets its carriage
//! returns; our stderr pipe does not, so each `\n` moved the cursor down but
//! not back to the left edge, and pacman's `ignoring package upgrade`
//! warnings came out as a staircase. A CR before the LF is invisible on a
//! terminal in its normal mode, so the translation is safe either way. When
//! stderr is a file or a pipe, bytes pass through untouched.

use std::collections::VecDeque;
use std::io::{Read, Write};
use std::process::{Command, ExitStatus, Stdio};

/// How much of the tail of stderr is kept. The reason is always near the end;
/// a full build log from an AUR helper can run to megabytes.
const TAIL_BYTES: usize = 8 * 1024;
/// Longest reason written to the log. A reason is one line a person reads in
/// a spreadsheet cell, not a transcript.
const MAX_REASON: usize = 200;

pub struct Handoff {
    pub status: ExitStatus,
    /// The line that best explains a failure. Always `None` on success.
    pub reason: Option<String>,
}

/// Run `cmd` with stderr relayed live, and return its status plus a reason
/// when it did not succeed.
///
/// Panics only if the program cannot be launched at all, matching every
/// `.status().unwrap_or_else(panic)` call this replaces.
pub fn run(cmd: &mut Command, what: &str) -> Handoff {
    use std::io::IsTerminal;
    let crlf = std::io::stderr().is_terminal();
    run_into(cmd, what, std::io::stderr(), crlf)
}

/// v1.5.8 (F-8): run with stderr left on the terminal, for the steps where
/// pacman asks a question. Relaying stderr through nog (v1.4.3, #21) let it
/// race pacman's stdout, which reaches the terminal through sudo's
/// pseudo-terminal: the question could land above the package table it asks
/// about and scroll out of sight. With both streams on the terminal they
/// arrive in pacman's own order. The cost: no failure reason is kept for
/// these steps — the status is, and the reason stays on screen.
pub fn run_on_screen(cmd: &mut Command, what: &str) -> Handoff {
    let status = cmd
        .status()
        .unwrap_or_else(|e| panic!("nog: failed to launch {}: {}", what, e));
    Handoff { status, reason: None }
}

/// `run`, relaying to any sink. The terminal in real use; in tests, a sink
/// that records *when* bytes arrive, which is the property that matters.
fn run_into<W: Write + Send + 'static>(cmd: &mut Command, what: &str, mut out: W, crlf: bool) -> Handoff {
    let mut child = cmd
        .stderr(Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("nog: failed to launch {}: {}", what, e));

    let mut pipe = child.stderr.take().expect("stderr was piped");
    let relay = std::thread::spawn(move || {
        let mut tail: VecDeque<u8> = VecDeque::with_capacity(TAIL_BYTES);
        let mut buf = [0u8; 4096];
        let mut after_cr = false;
        loop {
            match pipe.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if crlf {
                        let _ = out.write_all(&to_crlf(&buf[..n], &mut after_cr));
                    } else {
                        let _ = out.write_all(&buf[..n]);
                    }
                    let _ = out.flush();
                    for &b in &buf[..n] {
                        if tail.len() == TAIL_BYTES {
                            tail.pop_front();
                        }
                        tail.push_back(b);
                    }
                }
            }
        }
        tail.into_iter().collect::<Vec<u8>>()
    });

    let status = child
        .wait()
        .unwrap_or_else(|e| panic!("nog: failed to wait for {}: {}", what, e));
    let tail = relay.join().unwrap_or_default();
    let reason = if status.success() { None } else { reason_from(&tail) };
    Handoff { status, reason }
}

/// Turn every bare `\n` into `\r\n`, leaving an existing `\r\n` alone.
/// `after_cr` carries the last byte's state across reads, so a CR at the end
/// of one chunk and its LF at the start of the next stay one line ending.
fn to_crlf(chunk: &[u8], after_cr: &mut bool) -> Vec<u8> {
    let mut out = Vec::with_capacity(chunk.len() + chunk.len() / 16);
    for &b in chunk {
        if b == b'\n' && !*after_cr {
            out.push(b'\r');
        }
        out.push(b);
        *after_cr = b == b'\r';
    }
    out
}

/// Pick the line that explains the failure out of the end of stderr.
///
/// pacman and the AUR helpers put the cause on an `error:` line and then
/// usually add a summary (`Errors occurred, no packages were upgraded.`) that
/// says less. So the last `error:` line wins; failing that, the last line
/// with anything on it — which, when the user declined, is the question they
/// said no to.
pub fn reason_from(tail: &[u8]) -> Option<String> {
    let text = strip_ansi(&String::from_utf8_lossy(tail));
    let lines: Vec<&str> = text
        .split(|c| c == '\n' || c == '\r')
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect();
    let pick = lines
        .iter()
        .rev()
        .find(|l| {
            // pacman and the helpers say `error:`; makepkg, which the AUR
            // helpers run, says `==> ERROR:`. The trailing summary pacman adds
            // ("Errors occurred, ...") must not match: it names no cause.
            let l = l.to_ascii_lowercase();
            l.starts_with("error:") || l.starts_with("==> error:")
        })
        .or_else(|| lines.last())?;
    let mut s: String = pick.chars().take(MAX_REASON).collect();
    if pick.chars().count() > MAX_REASON {
        s.push('…');
    }
    Some(s)
}

/// Drop terminal colour and cursor sequences (`ESC [ … letter`), which pacman
/// and yay emit when colour is on and which would otherwise land in the log.
fn strip_ansi(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            if chars.peek() == Some(&'[') {
                chars.next();
                for n in chars.by_ref() {
                    if n.is_ascii_alphabetic() {
                        break;
                    }
                }
            }
            continue;
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_error_line_beats_the_summary_after_it() {
        let tail = b"(3/3) checking for file conflicts\n\
error: failed to commit transaction (conflicting files)\n\
foo: /usr/bin/foo exists in filesystem\n\
Errors occurred, no packages were upgraded.\n";
        assert_eq!(
            reason_from(tail).as_deref(),
            Some("error: failed to commit transaction (conflicting files)")
        );
    }

    #[test]
    fn a_makepkg_failure_inside_an_aur_build_is_found() {
        let tail = b"==> ERROR: A failure occurred in build().\n    Aborting...\n -> error making: discord\n";
        assert_eq!(
            reason_from(tail).as_deref(),
            Some("==> ERROR: A failure occurred in build().")
        );
    }

    #[test]
    fn a_declined_prompt_is_its_own_reason() {
        // pacman's question has no newline; the user's "n" went to the tty.
        let tail = b"Total Installed Size:  12.00 MiB\n\n:: Proceed with installation? [Y/n] ";
        assert_eq!(
            reason_from(tail).as_deref(),
            Some(":: Proceed with installation? [Y/n]")
        );
    }

    #[test]
    fn colour_and_progress_redraws_are_cleaned_out() {
        let tail = b"\x1b[1;31merror:\x1b[0m failed retrieving file 'x.pkg.tar.zst' from mirror\r\n";
        assert_eq!(
            reason_from(tail).as_deref(),
            Some("error: failed retrieving file 'x.pkg.tar.zst' from mirror")
        );
    }

    #[test]
    fn nothing_printed_means_no_reason() {
        assert_eq!(reason_from(b""), None);
        assert_eq!(reason_from(b"\n\n  \r\n"), None);
    }

    #[test]
    fn a_huge_line_is_cut_to_a_cell_sized_reason() {
        let long = format!("error: {}", "x".repeat(1000));
        let r = reason_from(long.as_bytes()).unwrap();
        assert_eq!(r.chars().count(), MAX_REASON + 1);
        assert!(r.ends_with('…'));
    }

    /// A sink that notes when its first byte arrived.
    struct Clock(std::sync::Arc<std::sync::Mutex<Option<std::time::Instant>>>, Vec<u8>);
    impl Write for Clock {
        fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
            let mut t = self.0.lock().unwrap();
            if t.is_none() && !b.is_empty() {
                *t = Some(std::time::Instant::now());
            }
            self.1.extend_from_slice(b);
            Ok(b.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn a_question_without_a_newline_reaches_the_user_immediately() {
        // pacman asks on stderr with no newline, then waits for an answer.
        // A relay that waited for a full line would show nothing until the
        // user had already answered a question they could not see. The child
        // here asks, then keeps the stream open for a full second: the
        // question must arrive long before that.
        let first = std::sync::Arc::new(std::sync::Mutex::new(None));
        let start = std::time::Instant::now();
        let h = run_into(
            Command::new("sh").args(["-c", "printf ':: Proceed with installation? [Y/n] ' >&2; sleep 1"]),
            "sh",
            Clock(first.clone(), Vec::new()),
            true,
        );
        assert!(h.status.success());
        let arrived = first.lock().unwrap().expect("the question never arrived");
        assert!(
            arrived - start < std::time::Duration::from_millis(500),
            "the question took {:?} to reach the user",
            arrived - start
        );
    }

    #[test]
    fn every_line_ending_returns_to_the_left_edge_on_a_terminal() {
        // F-1 of v1.5.2: under sudo's use_pty the user's terminal is raw, and
        // a bare LF leaves the cursor in its column — the staircase.
        let mut cr = false;
        assert_eq!(to_crlf(b"warning: a\nwarning: b\n", &mut cr), b"warning: a\r\nwarning: b\r\n");
        // Already CR LF: untouched, never doubled.
        let mut cr = false;
        assert_eq!(to_crlf(b"x\r\ny\r\n", &mut cr), b"x\r\ny\r\n");
        // A CR LF split across two reads stays one line ending.
        let mut cr = false;
        let mut both = to_crlf(b"x\r", &mut cr);
        both.extend(to_crlf(b"\ny\n", &mut cr));
        assert_eq!(both, b"x\r\ny\r\n");
        // A progress redraw (CR alone) passes as it is.
        let mut cr = false;
        assert_eq!(to_crlf(b"10%\r20%\r", &mut cr), b"10%\r20%\r");
    }

    #[test]
    fn the_relay_translates_for_a_terminal_and_not_for_a_file() {
        use std::sync::{Arc, Mutex};
        struct Keep(Arc<Mutex<Vec<u8>>>);
        impl Write for Keep {
            fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
                self.0.lock().unwrap().extend_from_slice(b);
                Ok(b.len())
            }
            fn flush(&mut self) -> std::io::Result<()> { Ok(()) }
        }
        let script = "printf 'warning: a\\nwarning: b\\n' >&2";
        let term = Arc::new(Mutex::new(Vec::new()));
        run_into(Command::new("sh").args(["-c", script]), "sh", Keep(term.clone()), true);
        assert_eq!(&*term.lock().unwrap(), b"warning: a\r\nwarning: b\r\n");
        let file = Arc::new(Mutex::new(Vec::new()));
        run_into(Command::new("sh").args(["-c", script]), "sh", Keep(file.clone()), false);
        assert_eq!(&*file.lock().unwrap(), b"warning: a\nwarning: b\n");
    }

    #[test]
    fn a_failing_command_reports_its_stderr_and_a_passing_one_reports_nothing() {
        // Real processes, so the relay thread and the tail are exercised
        // rather than only the text picking. Relayed into a sink, not the
        // real stderr: the fake error below once landed in every package
        // build log, where it read as a real one (#24).
        let bad = run_into(
            Command::new("sh").args(["-c", "echo 'error: no space left' >&2; exit 1"]),
            "sh",
            std::io::sink(),
            false,
        );
        assert!(!bad.status.success());
        assert_eq!(bad.reason.as_deref(), Some("error: no space left"));

        let good = run_into(
            Command::new("sh").args(["-c", "echo 'warning: noisy but fine' >&2"]),
            "sh",
            std::io::sink(),
            false,
        );
        assert!(good.status.success());
        assert_eq!(good.reason, None);
    }
}
