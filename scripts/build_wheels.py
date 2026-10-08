"""Repack dist/ release archives as platform wheels; never uploads or publishes."""
import argparse
import base64
import hashlib
import json
import re
import subprocess
import sys
import tarfile
import tempfile
import zipfile
from pathlib import Path

root = Path(__file__).resolve().parents[1]
fixed_tags = {
    'aarch64-apple-darwin': 'macosx_14_0_arm64',
    'x86_64-pc-windows-msvc': 'win_amd64',
}
host_targets = {
    'linux': 'x86_64-unknown-linux-gnu',
    'darwin': 'aarch64-apple-darwin',
    'win32': 'x86_64-pc-windows-msvc',
}
archive_name = re.compile(r'^gsu-v(?P<version>.+?)-(?P<target>\S+?)\.(?:tar\.gz|zip)$')


def extract(archive):
    names = {}
    if archive.name.endswith('.zip'):
        with zipfile.ZipFile(archive) as file:
            for info in file.infolist():
                names[Path(info.filename).name] = file.read(info)
    else:
        with tarfile.open(archive) as file:
            for member in file.getmembers():
                if member.isfile():
                    names[Path(member.name).name] = file.extractfile(member).read()
    binary = names.pop('gsu.exe' if 'windows' in archive.name else 'gsu', None)
    if binary is None:
        raise SystemExit(f'{archive.name}: no gsu binary inside')
    build = json.loads(names['BUILD.json'])
    return binary, build, names


def check_binary(binary, version, target, build):
    if build['version'] != version or build['target'] != target:
        raise SystemExit(f'BUILD.json mismatch: {build["version"]}/{build["target"]} != {version}/{target}')
    if host_targets.get(sys.platform) != target:
        return
    with tempfile.TemporaryDirectory() as tmp:
        path = Path(tmp) / 'gsu'
        path.write_bytes(binary)
        path.chmod(0o755)
        actual = subprocess.check_output([str(path), '--version'], text=True).strip()
    if actual != f'gsu {version}':
        raise SystemExit(f'Binary version mismatch: expected gsu {version}, got {actual}')


def record_line(path, data):
    digest = base64.urlsafe_b64encode(hashlib.sha256(data).digest()).rstrip(b'=').decode()
    return f'{path},sha256={digest},{len(data)}'


def build_wheel(archive, version, target, tag, out_dir):
    python_dir = root / 'python' / 'gsu'
    init_source = (python_dir / '__init__.py').read_text()
    init_source, count = re.subn(
        r'^__version__ = "[^"]+"$', f'__version__ = "{version}"',
        init_source, count=1, flags=re.MULTILINE)
    if count != 1:
        raise SystemExit('python/gsu/__init__.py: __version__ line not found')
    binary, build, extras = extract(archive)
    check_binary(binary, version, target, build)
    dist_info = f'gsu-{version}.dist-info'
    metadata = '\n'.join([
        'Metadata-Version: 2.1',
        'Name: gsu',
        f'Version: {version}',
        'Summary: Standalone, read-only PyTorch performance static checker',
        'License: MIT',
        'Requires-Python: >=3.9',
        'Description-Content-Type: text/markdown',
        '',
    ]) + '\n' + (root / 'README.md').read_text()
    files = {
        'gsu/__init__.py': init_source.encode(),
        'gsu/__main__.py': (python_dir / '__main__.py').read_bytes(),
        f'gsu/bin/gsu{".exe" if target.endswith("windows-msvc") else ""}': binary,
        f'{dist_info}/METADATA': metadata.encode(),
        f'{dist_info}/WHEEL': f'Wheel-Version: 1.0\nGenerator: build_wheels.py\nRoot-Is-Purelib: false\nTag: py3-none-{tag}\n'.encode(),
        f'{dist_info}/entry_points.txt': b'[console_scripts]\ngsu = gsu.__main__:main\n',
    }
    for name in ['LICENSE', 'THIRD_PARTY_NOTICES.txt']:
        if name not in extras:
            raise SystemExit(f'{archive.name}: missing {name}')
        files[f'{dist_info}/{name}'] = extras[name]
    wheel = out_dir / f'gsu-{version}-py3-none-{tag}.whl'
    with zipfile.ZipFile(wheel, 'w', zipfile.ZIP_DEFLATED) as file:
        record = []
        for path, data in sorted(files.items()):
            info = zipfile.ZipInfo(path)
            info.external_attr = (0o755 if path.startswith('gsu/bin/') else 0o644) << 16
            file.writestr(info, data)
            record.append(record_line(path, data))
        record.append(f'{dist_info}/RECORD,,')
        file.writestr(f'{dist_info}/RECORD', '\n'.join(record) + '\n')
    return wheel


def main():
    parser = argparse.ArgumentParser(
        description='Repack dist/ release archives as platform wheels.')
    parser.add_argument('--dist-dir', type=Path, default=root / 'dist')
    parser.add_argument('--out-dir', type=Path, default=root / 'dist' / 'wheels')
    parser.add_argument('--linux-tag', default='manylinux_2_28_x86_64',
                        help='platform tag for x86_64-unknown-linux-gnu archives')
    args = parser.parse_args()
    cargo_version = re.search(r'^version = "([^"]+)"$',
                              (root / 'Cargo.toml').read_text(), re.MULTILINE).group(1)
    py_version = re.search(r'^version = "([^"]+)"$',
                           (root / 'python' / 'pyproject.toml').read_text(), re.MULTILINE).group(1)
    if py_version != cargo_version:
        raise SystemExit(f'Version mismatch: python/pyproject.toml has {py_version}, Cargo.toml has {cargo_version}')
    wheels = []
    for archive in sorted(list(args.dist_dir.glob('*.tar.gz')) + list(args.dist_dir.glob('*.zip'))):
        match = archive_name.match(archive.name)
        if not match:
            continue
        version, target = match['version'], match['target']
        if version != cargo_version:
            print(f'Skipping {archive.name}: version {version} != Cargo.toml {cargo_version}', file=sys.stderr)
            continue
        if target == 'x86_64-unknown-linux-gnu':
            tag = args.linux_tag
        elif target in fixed_tags:
            tag = fixed_tags[target]
        else:
            raise SystemExit(f'{archive.name}: unsupported target {target}')
        args.out_dir.mkdir(parents=True, exist_ok=True)
        wheels.append(build_wheel(archive, version, target, tag, args.out_dir))
    if not wheels:
        raise SystemExit(f'No release archives found in {args.dist_dir}')
    for wheel in wheels:
        print(wheel)


if __name__ == '__main__':
    main()
