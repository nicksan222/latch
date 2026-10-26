#!/usr/bin/env python3
"""Manage only this checkout's virtual desktop; never signal an unverified PID."""
import argparse
import fcntl
import json
import os
from pathlib import Path
import signal
import socket
import subprocess
import time
import urllib.request

REPO = Path(__file__).resolve().parents[3]
STATE = REPO / ".dev/desktop"
URL = "http://127.0.0.1:5900/vnc.html?autoconnect=1&reconnect=1&resize=scale"
ENV = {**os.environ, "DISPLAY": ":99"}
CHILDREN = {}


def identity(pid):
    """Linux start time prevents a recycled PID from identifying another process."""
    try:
        path = Path(f"/proc/{pid}")
        stat = (path / "stat").read_text().rsplit(")", 1)[1].split()
        if path.stat().st_uid == os.getuid() and stat[0] != "Z":
            return stat[19]
    except (OSError, IndexError):
        pass
    return None


def managed(name):
    try:
        record = json.loads((STATE / f"{name}.json").read_text())
        pid = record["pid"]
        if pid > 1 and record["start"] is not None and identity(pid) == record["start"]:
            return pid
    except (OSError, ValueError, KeyError, TypeError):
        pass
    return None


def group_alive(pgid):
    for path in Path("/proc").iterdir():
        if not path.name.isdecimal():
            continue
        try:
            stat = (path / "stat").read_text().rsplit(")", 1)[1].split()
            if path.stat().st_uid == os.getuid() and stat[0] != "Z" and int(stat[2]) == pgid:
                return True
        except (OSError, IndexError, ValueError):
            continue
    return False


def launch(name, command, ready=None):
    pid = managed(name)
    if pid:
        if ready and not ready():
            raise RuntimeError(f"{name} is running but unhealthy; see {STATE / (name + '.log')}")
        return
    with (STATE / f"{name}.log").open("ab") as log:
        proc = subprocess.Popen(command, cwd=REPO, env=ENV, stdin=subprocess.DEVNULL,
                                stdout=log, stderr=log, start_new_session=True)
    CHILDREN[proc.pid] = proc
    (STATE / f"{name}.json").write_text(json.dumps({"pid": proc.pid, "start": identity(proc.pid)}))
    if ready:
        for _ in range(100):
            if proc.poll() is not None:
                break
            if ready():
                return
            time.sleep(0.1)
        if managed(name):
            os.killpg(proc.pid, signal.SIGTERM)
        raise RuntimeError(f"{name} did not start; see {STATE / (name + '.log')}")


def port_open(port):
    try:
        with socket.create_connection(("127.0.0.1", port), timeout=0.2):
            return True
    except OSError:
        return False


def display_ready():
    return subprocess.run(["xdpyinfo"], env=ENV, stdout=subprocess.DEVNULL,
                          stderr=subprocess.DEVNULL).returncode == 0


def window():
    result = subprocess.run(["xdotool", "search", "--onlyvisible", "--class", "latch"],
                            env=ENV, capture_output=True, text=True)
    return result.stdout.splitlines() if result.returncode == 0 else []


def start():
    # Do not silently attach to another checkout's display or network services.
    if not managed("display") and display_ready():
        raise RuntimeError("Display :99 is already in use by an unmanaged desktop")
    for name, port in (("vnc", 5901), ("novnc", 5900)):
        if not managed(name) and port_open(port):
            raise RuntimeError(f"Port {port} is already in use by an unmanaged service")
    launch("display", ["Xvfb", ":99", "-screen", "0", "1440x900x24", "-nolisten", "tcp", "-ac"], display_ready)
    launch("wm", ["openbox", "--sm-disable"])
    launch("vnc", ["x11vnc", "-display", ":99", "-rfbport", "5901", "-localhost",
                   "-forever", "-shared", "-nopw", "-xkb"], lambda: port_open(5901))
    web = next((str(p) for p in (Path("/usr/share/novnc"), Path("/usr/share/noVNC"))
                if (p / "vnc.html").is_file()), None)
    if not web:
        raise RuntimeError("noVNC is missing; rebuild the devcontainer")
    launch("novnc", ["websockify", "--web", web, "0.0.0.0:5900", "127.0.0.1:5901"], lambda: port_open(5900))
    with urllib.request.urlopen(URL, timeout=5) as response:
        if response.status != 200:
            raise RuntimeError("noVNC returned an unsuccessful HTTP status")
    print(URL)


def stop(app_only=False):
    for name in (["app"] if app_only else ["app", "novnc", "vnc", "wm", "display"]):
        pid = managed(name)
        if pid:
            os.killpg(pid, signal.SIGTERM)
            for _ in range(100):
                if not group_alive(pid):
                    break
                time.sleep(0.1)
            else:
                raise RuntimeError(f"{name} is still stopping; retry shortly")
        (STATE / f"{name}.json").unlink(missing_ok=True)
        if pid in CHILDREN:
            CHILDREN.pop(pid).wait(timeout=1)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["start", "app", "wait", "status", "stop", "stop-app", "shot", "click", "type", "key"])
    parser.add_argument("args", nargs="*")
    parser.add_argument("--timeout", type=float, default=900)
    options = parser.parse_args()
    STATE.mkdir(parents=True, exist_ok=True, mode=0o700)
    # Serialize starts and stops; children never inherit this lock.
    with (STATE / "control.lock").open("w") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX)
        if options.command == "start":
            start()
        elif options.command == "app":
            start()
            launch("app", ["bash", str(Path(__file__).with_name("app.sh"))])
            print(f"Latch starting; build/app log: {STATE / 'app.log'}")
        elif options.command in ("stop", "stop-app"):
            stop(options.command == "stop-app")
        elif options.command == "status":
            print(json.dumps({name: managed(name) for name in ("display", "wm", "vnc", "novnc", "app")}))
            print(f"Visible Latch windows: {window()}")
        elif options.command == "wait":
            fcntl.flock(lock, fcntl.LOCK_UN)
            deadline = time.monotonic() + options.timeout
            while not window():
                if not managed("app"):
                    raise RuntimeError(f"Latch exited; see {STATE / 'app.log'}")
                if time.monotonic() >= deadline:
                    raise RuntimeError(f"Timed out waiting for Latch; see {STATE / 'app.log'}")
                time.sleep(0.5)
            print("Latch window ready")
        else:
            args = options.args
            if options.command == "shot":
                destination = Path(args[0]) if args else STATE / "screenshot.png"
                subprocess.run(["scrot", "--overwrite", str(destination)], env=ENV, check=True)
                print(destination)
            elif options.command == "click" and len(args) == 2:
                subprocess.run(["xdotool", "mousemove", "--sync", *args, "click", "1"], env=ENV, check=True)
            elif options.command == "type" and len(args) == 1:
                subprocess.run(["xdotool", "type", "--clearmodifiers", "--", args[0]], env=ENV, check=True)
            elif options.command == "key" and len(args) == 1:
                subprocess.run(["xdotool", "key", "--clearmodifiers", args[0]], env=ENV, check=True)
            else:
                parser.error("click needs X Y; type/key need one quoted argument")


if __name__ == "__main__":
    try:
        main()
    except (RuntimeError, OSError, subprocess.CalledProcessError) as error:
        raise SystemExit(str(error)) from None
