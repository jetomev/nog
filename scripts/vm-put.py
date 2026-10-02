#!/usr/bin/env python3
"""Copy one file into a test VM through the qemu guest agent.

    scripts/vm-put.py [--vm NAME] [--mode 755] SRC DST

Writes in 64 KiB chunks (the agent's messages have a size limit), then sets
the mode and checks the sha256 on both sides. Used with vm-exec.py to try a
fresh `target/release/nog` in the VM before a release.
"""
import argparse, base64, hashlib, json, subprocess, sys

p = argparse.ArgumentParser()
p.add_argument("--vm", default="kognog-hypeforge")
p.add_argument("--mode", default="755")
p.add_argument("src")
p.add_argument("dst")
a = p.parse_args()


def agent(payload):
    out = subprocess.run(
        ["virsh", "-c", "qemu:///system", "qemu-agent-command", a.vm, json.dumps(payload)],
        capture_output=True, text=True, check=True).stdout
    return json.loads(out)["return"]


data = open(a.src, "rb").read()
h = agent({"execute": "guest-file-open", "arguments": {"path": a.dst, "mode": "wb"}})
for i in range(0, len(data), 1 << 16):
    chunk = base64.b64encode(data[i:i + (1 << 16)]).decode()
    agent({"execute": "guest-file-write", "arguments": {"handle": h, "buf-b64": chunk}})
agent({"execute": "guest-file-close", "arguments": {"handle": h}})

r = subprocess.run([sys.executable, __file__.replace("vm-put.py", "vm-exec.py"), "--vm", a.vm,
                    f"chmod {a.mode} '{a.dst}' && sha256sum '{a.dst}'"],
                   capture_output=True, text=True).stdout
local = hashlib.sha256(data).hexdigest()
print(f"local  {local}\nremote {r.split()[0] if r.strip() else '?'}")
sys.exit(0 if local in r else 1)
