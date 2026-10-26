#!/usr/bin/env python3
"""Seed only this checkout's .dev/latch; inherited Latch paths are ignored."""
import os
from pathlib import Path
import subprocess


def seed_environment(project):
    root = project / '.dev' / 'latch'
    # Refuse redirected ancestors before creating or opening anything.
    for path in (project / '.dev', root, *(root / n for n in ('config', 'data', 'runtime', 'ram', 'vault'))):
        if path.is_symlink():
            raise RuntimeError(f'Refusing symlink: {path}')
    env = os.environ.copy()
    for name in ('config', 'data', 'runtime', 'ram'):
        env[f'LATCH_{name.upper()}_DIR'] = str(root / name)
    env.pop('LATCH_ALLOW_WEAK_KDF', None)
    # Do not let inherited Git repository/index/config overrides escape isolation.
    for key in list(env):
        if key.startswith('GIT_'):
            del env[key]
    env['GIT_CONFIG_GLOBAL'] = os.devnull
    env['GIT_CONFIG_NOSYSTEM'] = '1'
    return env


if __name__ == '__main__':
    project = Path(__file__).resolve().parents[2]
    subprocess.run(['cargo', 'run', '--manifest-path', str(project / 'Cargo.toml'), '--package', 'latch-core',
                    '--no-default-features', '--example', 'seed_dev'],
                   cwd=project, env=seed_environment(project), check=True)
