#!/usr/bin/env python3
"""Retain immutable chunks for open tabs for at least seven days, plus active/previous releases."""
import pathlib
import re
import shutil
import time

root = pathlib.Path('/srv/spellgambit/web').resolve(strict=True)
releases = root / 'releases'
active = (root / 'current').resolve(strict=True)
keep = {active}
completed = []
for directory in releases.iterdir():
    if directory.is_symlink() or not directory.is_dir() or not re.fullmatch('[0-9a-f]{40}', directory.name):
        continue
    if not (directory / '.complete').is_file():
        continue
    completed.append(directory)
completed.sort(key=lambda p: (p / '.complete').stat().st_mtime, reverse=True)
keep.update(completed[:2])
threshold = time.time() - 7 * 86400
for directory in completed:
    if (directory / '.complete').stat().st_mtime >= threshold:
        keep.add(directory)
for directory in completed:
    if directory not in keep:
        if directory.resolve().parent != releases.resolve():
            raise SystemExit('Release directory outside deployment root')
        shutil.rmtree(directory)
assets = {p.name for directory in keep for p in (directory / 'assets').iterdir()}
for asset in (root / 'assets').iterdir():
    if asset.is_symlink():
        raise SystemExit('Unexpected asset symlink')
    if asset.is_file() and asset.name not in assets and asset.stat().st_mtime < threshold:
        asset.unlink()
print(f'Retained {len(keep)} releases and their immutable chunks')
