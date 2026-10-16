"""Run in a disposable Linux container with /srv on tmpfs, never on a real host."""
import hashlib
import io
import json
import os
import pathlib
import subprocess
import tarfile
import time

root = pathlib.Path('/srv/spellgambit/web')
assert not root.exists(), 'Fixture requires a fresh container filesystem'
root.mkdir(parents=True)


def archive(release, asset='shared.js', content=b'original', extra=None):
    files = {'index.html': b'fixture', 'sw.js': b'worker',
             '.vite/release.json': json.dumps({'release': release}).encode(),
             f'assets/{asset}': content}
    files.update(extra or {})
    output = io.BytesIO()
    with tarfile.open(fileobj=output, mode='w:gz') as tar:
        for name, data in files.items():
            item = tarfile.TarInfo(name)
            item.size = len(data)
            tar.addfile(item, io.BytesIO(data))
    return output.getvalue()


def install(release, payload, success):
    result = subprocess.run(['python3', '/deploy/install-web.py', release],
                            input=payload, capture_output=True)
    assert (result.returncode == 0) == success, result.stderr.decode()
    if not success:
        assert not (root/'releases'/release/'.complete').exists()


sha = 'a' * 40
install(sha, archive(sha), True)
before = hashlib.sha256((root/'assets/shared.js').read_bytes()).digest()
duplicate = subprocess.run(['python3', '/deploy/install-web.py', sha],
                           input=archive(sha), capture_output=True)
assert duplicate.returncode != 0
install('b'*40, archive('b'*40, extra={'../../escape': b'bad'}), False)
assert not (root/'escape').exists()
install('c'*40, archive('c'*40, extra={'.complete': b'forged'}), False)
install('d'*40, archive('wrong-version'), False)
install('e'*40, archive('e'*40, content=b'collision'), False)
assert hashlib.sha256((root/'assets/shared.js').read_bytes()).digest() == before

# Link entries must not escape into the release tree.
payload = io.BytesIO()
with tarfile.open(fileobj=payload, mode='w:gz') as tar:
    link = tarfile.TarInfo('link')
    link.type = tarfile.SYMTYPE
    link.linkname = '/etc'
    tar.addfile(link)
install('f'*40, payload.getvalue(), False)

now = time.time()
for number, age in [(1, 0), (2, 86400), (3, 8*86400), (4, 10*86400)]:
    release = str(number) * 40
    name = f'unique-{number}.js'
    install(release, archive(release, name), True)
    timestamp = now - age
    os.utime(root/'releases'/release/'.complete', (timestamp, timestamp))
    os.utime(root/'assets'/name, (timestamp, timestamp))
os.symlink('releases/'+'3'*40, root/'current')
subprocess.run(['python3', '/deploy/prune-web.py'], check=True)
assert (root/'releases'/('3'*40)).is_dir(), 'Active release must survive retention'
assert (root/'assets/unique-3.js').is_file()
assert (root/'releases'/('1'*40)).is_dir()
assert not (root/'releases'/('4'*40)).exists()
assert not (root/'assets/unique-4.js').exists()
assert (root/'releases'/('b'*40)).is_dir(), 'Incomplete releases are retained for inspection'
print('Web installation: traversal, links, forged marker, version, collisions and retention passed')
