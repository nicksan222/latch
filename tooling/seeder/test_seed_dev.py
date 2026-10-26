#!/usr/bin/env python3
import importlib.util
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location('seed_dev', Path(__file__).with_name('seed-dev.py'))
seed_dev = importlib.util.module_from_spec(spec)
spec.loader.exec_module(seed_dev)


class SeedEnvironmentTests(unittest.TestCase):
    def test_overrides_real_paths_and_git_injection(self):
        with tempfile.TemporaryDirectory() as directory:
            project = Path(directory)
            inherited = {'LATCH_CONFIG_DIR': '/real/config', 'LATCH_DATA_DIR': '/real/data',
                         'LATCH_ALLOW_WEAK_KDF': '1', 'GIT_DIR': '/real/vault/.git',
                         'GIT_CONFIG_COUNT': '1', 'GIT_CONFIG_KEY_0': 'core.hooksPath'}
            with patch.dict(os.environ, inherited):
                env = seed_dev.seed_environment(project)
            for name in ('config', 'data', 'runtime', 'ram'):
                self.assertEqual(env[f'LATCH_{name.upper()}_DIR'], str(project / '.dev/latch' / name))
            for key in inherited:
                if key.startswith('GIT_') or key == 'LATCH_ALLOW_WEAK_KDF':
                    self.assertNotIn(key, env)
            self.assertFalse((project / '.dev').exists())

    def test_refuses_symlinked_development_directory(self):
        with tempfile.TemporaryDirectory() as directory:
            project = Path(directory)
            (project / '.dev').symlink_to(project, target_is_directory=True)
            with self.assertRaises(RuntimeError):
                seed_dev.seed_environment(project)


if __name__ == '__main__':
    unittest.main()
