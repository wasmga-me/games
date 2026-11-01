#!/usr/bin/env python3
"""Run mechanics, loss/retry, save and campaign checks for the ad-style games."""
import os
from pathlib import Path
import subprocess
import sys
import tempfile

root = Path(__file__).resolve().parent
names = ['pinwork', 'decant', 'unpark', 'higher-floor', 'flowline', 'boarding', 'unbolt', 'hive-guard', 'offgrid', 'hexpile', 'bankshot']
if len(sys.argv) > 1:
    unknown = set(sys.argv[1:]) - set(names)
    if unknown:
        sys.exit(f'Unknown games: {", ".join(sorted(unknown))}')
    names = sys.argv[1:]
failed = []
for game in names:
    print(f'Checking {game}', flush=True)
    env = dict(os.environ)
    env.setdefault('CARGO_TARGET_DIR', str(Path(tempfile.gettempdir()) / 'wasmgame-ad-tests' / game))
    result = subprocess.run(['cargo', 'test', '--locked', '--release', '--manifest-path', str(root / game / 'Cargo.toml')], env=env)
    if result.returncode:
        failed.append(game)
if failed:
    sys.exit(f'Failed: {", ".join(failed)}')
print(f'All {len(names)} game suites passed.')
