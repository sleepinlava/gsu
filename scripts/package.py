"""Package locally verified binaries; never uploads or publishes."""
import hashlib
import json
import platform
import re
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tarfile
import zipfile
root = Path(__file__).resolve().parents[1]
target = sys.argv[1]
if target not in ['x86_64-unknown-linux-gnu', 'aarch64-apple-darwin', 'x86_64-pc-windows-msvc']:
    raise SystemExit('Unsupported package target')
version = re.search(r'^version = "([^"]+)"$', (root / 'Cargo.toml').read_text(), re.MULTILINE).group(1)
name = f'gsu-v{version}-{target}'
staging = root / 'target' / 'packages' / name
staging.mkdir(parents=True, exist_ok=True)
binary = root / 'target' / 'release' / ('gsu.exe' if 'windows' in target else 'gsu')
actual = subprocess.check_output([str(binary), '--version'], text=True).strip()
if actual != f'gsu {version}':
    raise SystemExit(f'Binary version mismatch: expected gsu {version}, got {actual}')
shutil.copy2(binary, staging / binary.name)
for filename in ['LICENSE', 'THIRD_PARTY_NOTICES.txt']:
    shutil.copy2(root / filename, staging / filename)
shutil.copy2(root / 'schemas' / 'diagnostics-v2.schema.json', staging / 'diagnostics-v2.schema.json')
(staging / 'INSTALL.txt').write_text('Copy gsu (gsu.exe on Windows) to a directory on your PATH.\nRun gsu --version and gsu check .\nNo Python, PyTorch, CUDA, or Rust installation is required at runtime.\nThis is a candidate package; consult the release acceptance record.\n')
(staging / 'BUILD.json').write_text(json.dumps({
    'version': version, 'target': target, 'build_platform': platform.platform(),
    'binary_sha256': hashlib.sha256(binary.read_bytes()).hexdigest(),
    'cargo_lock_sha256': hashlib.sha256((root / 'Cargo.lock').read_bytes()).hexdigest(),
    'source_sha256': hashlib.sha256(b''.join(p.relative_to(root).as_posix().encode() + p.read_bytes() for p in sorted((root / 'src').rglob('*.rs')))).hexdigest(),
    'toolchain': '1.94.0', 'acceptance': 'development candidate; see docs/release/status.md'
}, indent=2) + '\n')
output = root / 'dist' 
output.mkdir(exist_ok=True)
if 'windows' in target:
    archive = output / (name + '.zip')
    with zipfile.ZipFile(archive, 'w', zipfile.ZIP_DEFLATED) as file:
        for item in sorted(staging.iterdir()):
            file.write(item, name + '/' + item.name)
else:
    archive = output / (name + '.tar.gz')
    with tarfile.open(archive, 'w:gz') as file:
        file.add(staging, arcname=name)
checksum = hashlib.sha256(archive.read_bytes()).hexdigest()
(output / (name + '.sha256')).write_text(checksum + '  ' + archive.name + '\n')
print(archive)
if os.environ.get('GITHUB_OUTPUT'):
    with open(os.environ['GITHUB_OUTPUT'], 'a') as output_file:
        output_file.write(f'archive={archive.relative_to(root).as_posix()}\n')
        output_file.write(f'checksum={(output / (name + ".sha256")).relative_to(root).as_posix()}\n')
