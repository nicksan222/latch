"""PID reuse and desktop ownership must never kill unrelated processes."""
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location("desktop", Path(__file__).with_name("desktop.py"))
desktop = importlib.util.module_from_spec(spec)
spec.loader.exec_module(desktop)


class LifecycleTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.state_patch = patch.object(desktop, "STATE", Path(self.temp.name))
        self.state_patch.start()

    def tearDown(self):
        self.state_patch.stop()
        self.temp.cleanup()

    def record(self, pid, start):
        (desktop.STATE / "app.json").write_text(json.dumps({"pid": pid, "start": start}))

    def test_recycled_pid_is_not_signaled(self):
        self.record(os.getpid(), "old-start-time")
        with patch.object(desktop.os, "killpg") as kill:
            desktop.stop(app_only=True)
            kill.assert_not_called()

    def test_dead_pid_with_null_start_is_not_managed(self):
        self.record(99999999, None)
        self.assertIsNone(desktop.managed("app"))

    def test_invalid_record_is_ignored(self):
        (desktop.STATE / "app.json").write_text("{broken")
        self.assertIsNone(desktop.managed("app"))

    def test_start_refuses_unmanaged_display(self):
        with patch.object(desktop, "display_ready", return_value=True), \
                patch.object(desktop, "launch") as launch:
            with self.assertRaisesRegex(RuntimeError, "unmanaged desktop"):
                desktop.start()
            launch.assert_not_called()

    def test_start_refuses_unmanaged_port(self):
        with patch.object(desktop, "display_ready", return_value=False), \
                patch.object(desktop, "port_open", return_value=True), \
                patch.object(desktop, "launch") as launch:
            with self.assertRaisesRegex(RuntimeError, "unmanaged service"):
                desktop.start()
            launch.assert_not_called()

    def test_managed_process_is_reused_and_stopped(self):
        command = ["sleep", "60"]
        desktop.launch("app", command)
        pid = desktop.managed("app")
        self.assertIsNotNone(pid)
        try:
            desktop.launch("app", command)
            self.assertEqual(desktop.managed("app"), pid)
        finally:
            desktop.stop(app_only=True)
        self.assertIsNone(desktop.managed("app"))
        # Reap the detached test child if it still belongs to this test process.
        try:
            os.waitpid(pid, 0)
        except ChildProcessError:
            pass

    def test_stop_waits_for_children_after_leader_exits(self):
        ready = desktop.STATE / "ready"
        finished = desktop.STATE / "finished"
        child = (
            "import signal,time,sys; from pathlib import Path; "
            f"signal.signal(signal.SIGTERM, lambda *_: (time.sleep(.3), Path({str(finished)!r}).touch(), sys.exit())); "
            f"Path({str(ready)!r}).touch(); time.sleep(60)"
        )
        parent = (
            "import subprocess,time; "
            f"subprocess.Popen([{sys.executable!r}, '-c', {child!r}]); time.sleep(60)"
        )
        desktop.launch("app", [sys.executable, "-c", parent])
        try:
            deadline = time.monotonic() + 5
            while not ready.exists() and time.monotonic() < deadline:
                time.sleep(.01)
            self.assertTrue(ready.exists())
        finally:
            desktop.stop(app_only=True)
        self.assertTrue(finished.exists())


if __name__ == "__main__":
    unittest.main()
