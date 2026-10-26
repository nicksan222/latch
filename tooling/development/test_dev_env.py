import json
import os
from pathlib import Path
import subprocess
import unittest

REPO = Path(__file__).resolve().parents[2]


class NativeEnvironmentTests(unittest.TestCase):
    def test_native_launches_override_real_storage_and_weak_kdf(self):
        inherited = {**os.environ, "LATCH_CONFIG_DIR": "/real/config",
                     "LATCH_DATA_DIR": "/real/data", "LATCH_RUNTIME_DIR": "/real/runtime",
                     "LATCH_RAM_DIR": "/real/ram", "LATCH_ALLOW_WEAK_KDF": "1"}
        script = "source tooling/development/dev-env.sh; python3 -c 'import json,os; print(json.dumps({k:os.getenv(k) for k in [\"LATCH_CONFIG_DIR\",\"LATCH_DATA_DIR\",\"LATCH_RUNTIME_DIR\",\"LATCH_RAM_DIR\",\"LATCH_ALLOW_WEAK_KDF\"]}))'"
        result = subprocess.run(["bash", "-c", script], cwd=REPO, env=inherited,
                                check=True, capture_output=True, text=True)
        env = json.loads(result.stdout)
        for name in ("config", "data", "runtime"):
            self.assertEqual(env[f"LATCH_{name.upper()}_DIR"], str(REPO / ".dev/latch" / name))
        self.assertIsNone(env["LATCH_ALLOW_WEAK_KDF"])
        self.assertRegex(env["LATCH_RAM_DIR"], rf"^/dev/shm/latch-dev-{os.getuid()}-[a-f0-9]{{12}}$")


if __name__ == "__main__":
    unittest.main()
