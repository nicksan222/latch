#!/usr/bin/env python3
"""Seed, drive, and capture the real native Latch desktop under Xvfb."""

from pathlib import Path
import os
import signal
import subprocess
import time

PROJECT = Path('/workspaces/latch')
OUTPUT = Path('/output')
PASS = 'correct horse battery staple'
FPS = int(os.environ.get('LATCH_DEMO_FPS', '10'))
WIDTH = int(os.environ.get('LATCH_DEMO_WIDTH', '1280'))
WINDOW_LEFT = 160
WINDOW_TOP = 100


def run(*args, quiet=True):
    kwargs = {'check': True}
    if quiet:
        kwargs.update(stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    return subprocess.run(args, **kwargs)


def xdo(*args):
    run('xdotool', *map(str, args))


def click(x, y, pause=0.5):
    # Coordinates are relative to the captured 1600x1000 app window.
    xdo('mousemove', WINDOW_LEFT + x, WINDOW_TOP + y, 'click', '1')
    time.sleep(pause)


def wait_for_window(timeout=30):
    deadline = time.monotonic() + timeout
    while time.monotonic() < deadline:
        found = subprocess.run(
            ['xdotool', 'search', '--onlyvisible', '--name', '^Latch$'],
            capture_output=True, text=True
        )
        if found.returncode == 0 and found.stdout.strip():
            return found.stdout.splitlines()[0]
        time.sleep(0.1)
    raise RuntimeError('Latch window did not appear')


def screenshot(name):
    run('scrot', '-o', '-u', str(OUTPUT / f'stage-{name}.png'))


def seed():
    seed_binary = Path(os.environ['LATCH_DEMO_SEED'])
    if not seed_binary.is_file():
        raise RuntimeError(f'seed binary not found: {seed_binary}')
    env = os.environ.copy()
    env['GIT_CONFIG_GLOBAL'] = os.devnull
    env['GIT_CONFIG_NOSYSTEM'] = '1'
    for key in list(env):
        if key.startswith('GIT_') and key not in ('GIT_CONFIG_GLOBAL', 'GIT_CONFIG_NOSYSTEM'):
            del env[key]
    subprocess.run([str(seed_binary)], cwd=PROJECT, env=env, check=True)


def main():
    OUTPUT.mkdir(parents=True, exist_ok=True)
    seed()
    wm = subprocess.Popen(['openbox'], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    log = (OUTPUT / 'app.log').open('wb')
    app = subprocess.Popen(
        [os.environ['LATCH_DEMO_BINARY']], cwd=PROJECT, stdout=log, stderr=subprocess.STDOUT
    )
    recorder = None
    try:
        window = wait_for_window()
        xdo('windowsize', window, 1600, 1000)
        xdo('windowmove', window, WINDOW_LEFT, WINDOW_TOP)
        xdo('windowactivate', '--sync', window)
        time.sleep(2)

        screenshot('locked')
        recorder = subprocess.Popen([
            'ffmpeg', '-hide_banner', '-loglevel', 'error', '-y',
            '-f', 'x11grab', '-framerate', str(FPS), '-video_size', '1600x1000',
            '-i', f':99.0+{WINDOW_LEFT},{WINDOW_TOP}', '-c:v', 'ffv1', str(OUTPUT / 'capture.mkv')
        ])
        time.sleep(1.4)
        xdo('type', '--clearmodifiers', '--delay', 32, PASS)
        time.sleep(0.5)
        xdo('key', 'Return')
        time.sleep(3.0)
        screenshot('unlocked')

        # Select the seeded login, reveal its fictional password, then copy it.
        click(410, 500, 1.2)
        screenshot('login')
        click(1420, 275, 1.2)
        click(1448, 275, 1.4)
        screenshot('secret')

        # Briefly switch profiles to make isolation visible, then return to Acme.
        click(90, 160, 1.4)
        screenshot('profile')
        click(90, 125, 1.2)

        # Open the saved environment and show the safe terminal command dialog.
        click(410, 225, 1.4)
        screenshot('environment')
        click(900, 240, 1.2)
        time.sleep(1.5)
        screenshot('environment-run')
    finally:
        if recorder:
            recorder.send_signal(signal.SIGINT)
            try:
                recorder.wait(timeout=10)
            except subprocess.TimeoutExpired:
                recorder.kill()
                recorder.wait()
        app.terminate()
        try:
            app.wait(timeout=10)
        except subprocess.TimeoutExpired:
            app.kill()
            app.wait()
        wm.terminate()
        try:
            wm.wait(timeout=5)
        except subprocess.TimeoutExpired:
            wm.kill()
        log.close()

    filter_graph = (
        f'fps={FPS},scale={WIDTH}:-2:flags=lanczos,split[s0][s1];'
        '[s0]palettegen=max_colors=192:stats_mode=diff[p];'
        '[s1][p]paletteuse=dither=bayer:bayer_scale=3:diff_mode=rectangle'
    )
    run(
        'ffmpeg', '-hide_banner', '-loglevel', 'error', '-y',
        '-i', str(OUTPUT / 'capture.mkv'),
        '-vf', filter_graph, '-loop', '0', str(OUTPUT / 'demo.gif'), quiet=False
    )
    print('Captured and encoded the real native Latch application.')


if __name__ == '__main__':
    main()
