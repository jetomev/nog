#!/usr/bin/env python3
"""Run one shell command inside a test VM through the qemu guest agent.

    scripts/vm-exec.py [--vm NAME] [--input TEXT] [--timeout S] -- 'command'

Runs as root in the guest via `bash -lc`. Prints stdout, then stderr, then the
exit code. --input is fed to the command's stdin (answers to prompts).
Used for the nog VM test matrices (v1.5.6 onwards).
"""
import argparse, base64, json, subprocess, sys, time

p = argparse.ArgumentParser()
p.add_argument("--vm", default="kognog-hypeforge")
p.add_argument("--input", default=None)
p.add_argument("--timeout", type=int, default=900)
p.add_argument("cmd")
a = p.parse_args()


def agent(payload):
    out = subprocess.run(
        ["virsh", "-c", "qemu:///system", "qemu-agent-command", a.vm, json.dumps(payload)],
        capture_output=True, text=True, check=True).stdout
    return json.loads(out)["return"]


args = {"path": "/bin/bash", "arg": ["-lc", a.cmd], "capture-output": True}
if a.input is not None:
    args["input-data"] = base64.b64encode(a.input.encode()).decode()
pid = agent({"execute": "guest-exec", "arguments": args})["pid"]

deadline = time.time() + a.timeout
while True:
    st = agent({"execute": "guest-exec-status", "arguments": {"pid": pid}})
    if st.get("exited"):
        break
    if time.time() > deadline:
        sys.exit(f"timed out after {a.timeout}s (guest pid {pid} still running)")
    time.sleep(2)

for key in ("out-data", "err-data"):
    if key in st:
        sys.stdout.write(base64.b64decode(st[key]).decode(errors="replace"))
print(f"\n[exit {st.get('exitcode')}]")
