#!/usr/bin/env python3
"""Run a command in a real terminal and answer its prompts as they appear.

    vm-drive.py [--timeout S] [--log FILE] 'command' REGEX ANSWER [REGEX ANSWER ...]

Copied into the test VM and run there (nog's v1.9.0 matrix). Typing every
answer ahead of time does not work: sudo and pacman discard typed-ahead
input when they start, so the second answer never arrives. This driver
watches the output and types each answer only once its prompt has been
printed. Every rule stays active (yay asks the same question per package);
rules are tried in the order given. The whole transcript goes to stdout
(and to --log), then "---- exit N". Exit code = the command's.
"""
import argparse, fcntl, os, pty, re, select, signal, struct, sys, termios, time

p = argparse.ArgumentParser()
p.add_argument("--timeout", type=int, default=1800)
p.add_argument("--log", default=None)
p.add_argument("cmd")
p.add_argument("rules", nargs="*")
a = p.parse_args()
if len(a.rules) % 2:
    sys.exit("rules come in pairs: REGEX ANSWER")
rules = [(re.compile(a.rules[i]), a.rules[i + 1]) for i in range(0, len(a.rules), 2)]

pid, fd = pty.fork()
if pid == 0:
    os.environ["TERM"] = "xterm-256color"
    os.execvp("/bin/bash", ["/bin/bash", "-lc", a.cmd])
fcntl.ioctl(fd, termios.TIOCSWINSZ, struct.pack("HHHH", 40, 120, 0, 0))

log = open(a.log, "wb") if a.log else None
transcript = bytearray()
pending = bytearray()           # output not yet matched against a rule
deadline = time.time() + a.timeout
status = None
while True:
    if time.time() > deadline:
        os.kill(pid, signal.SIGTERM)
        sys.stdout.write(transcript.decode(errors="replace"))
        sys.exit(f"\n---- timed out after {a.timeout}s")
    r, _, _ = select.select([fd], [], [], 1.0)
    if r:
        try:
            chunk = os.read(fd, 65536)
        except OSError:
            chunk = b""
        if not chunk:
            break
        transcript += chunk
        pending += chunk
        if log:
            log.write(chunk); log.flush()
        text = pending.decode(errors="replace")
        for rx, answer in rules:
            m = rx.search(text)
            if m:
                time.sleep(0.3)                      # let the prompt finish its line
                os.write(fd, (answer + "\n").encode())
                pending = bytearray(text[m.end():].encode())
                break
    else:
        wpid, st = os.waitpid(pid, os.WNOHANG)
        if wpid:
            status = st
            break
if status is None:
    _, status = os.waitpid(pid, 0)
code = os.waitstatus_to_exitcode(status)
sys.stdout.write(transcript.decode(errors="replace"))
sys.stdout.write(f"\n---- exit {code}\n")
sys.exit(code if code >= 0 else 1)
