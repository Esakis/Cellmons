"""Validate a CI archive before installing it. Never unpack links or overwrite a release."""
import hashlib
import json
import os
import pathlib
import re
import sys
import tarfile

root = pathlib.Path('/srv/spellgambit/web').resolve()
release = sys.argv[1]
if not re.fullmatch(r'[0-9a-f]{40}', release):
    raise SystemExit('Expected a commit SHA')
destination = root / 'releases' / release
if destination.exists():
    raise SystemExit('Release already installed; select it for rollback, do not overwrite it')
destination.mkdir(parents=True)
total = 0
members = 0
seen = set()
try:
    with tarfile.open(fileobj=sys.stdin.buffer, mode='r|gz') as archive:
        for member in archive:
            members += 1
            if members > 5000 or len(member.name)>300:
                raise ValueError('Oversized archive directory')
            name = pathlib.PurePosixPath(member.name.removeprefix('./'))
            if name.is_absolute() or '..' in name.parts or '\\' in str(name):
                raise ValueError('Unsafe archive path')
            if str(name) == '.complete':
                raise ValueError('Completion marker belongs to the installer')
            if member.isdir():
                continue
            if not member.isfile() or str(name) in seen:
                raise ValueError('Only unique regular files are allowed')
            seen.add(str(name))
            total += member.size
            if total > 64 * 1024 * 1024 or len(seen) > 5000 or member.size > 8 * 1024 * 1024:
                raise ValueError('Oversized web archive')
            target = destination / str(name)
            target.parent.mkdir(parents=True, exist_ok=True)
            data = archive.extractfile(member).read(member.size + 1)
            if len(data) != member.size:
                raise ValueError('Incomplete archive')
            target.write_bytes(data)
    if not (destination / 'index.html').is_file() or not (destination / 'sw.js').is_file():
        raise ValueError('Incomplete browser release')
    if json.loads((destination / '.vite/release.json').read_text())['release'] != release:
        raise ValueError('Web release does not match commit')
    # All Vite chunks are content addressed; retain old chunks for open browser tabs.
    for source in (destination / 'assets').iterdir():
        if not source.is_file():
            raise ValueError('Unexpected asset tree')
        target = root / 'assets' / source.name
        target.parent.mkdir(exist_ok=True)
        if target.exists():
            if hashlib.sha256(target.read_bytes()).digest() != hashlib.sha256(source.read_bytes()).digest():
                raise ValueError('Asset name collision')
        else:
            temporary = target.with_suffix(target.suffix + '.new')
            temporary.write_bytes(source.read_bytes())
            os.replace(temporary, target)
    (destination / '.complete').write_text(release + '\n', encoding='ascii')
except BaseException:
    # Incomplete directories are deliberately left for operator inspection, never selected.
    raise
print(f'Installed {release}: {len(seen)} files, {total} bytes')
