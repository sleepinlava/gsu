"""Collect versioned dependency licenses from Cargo's local source cache."""
import json
from pathlib import Path
import subprocess
import sys
cargo = sys.argv[1] if len(sys.argv) > 1 else 'cargo'
metadata = json.loads(subprocess.check_output([cargo, 'metadata', '--offline', '--locked', '--format-version', '1']))
output = ['GSU third-party dependency notices\nVersions correspond to Cargo.lock. Includes build and development dependencies.\n']
for package in sorted(metadata['packages'], key=lambda p: p['name']):
    if package['name'] == 'gsu':
        continue
    root = Path(package['manifest_path']).parent
    output.append('\n' + '=' * 72 + '\n' + package['name'] + ' ' + package['version'] + '\nLicense expression: ' + str(package['license']) + '\nSource: ' + str(package['repository'] or package['source']) + '\n')
    files = [f for f in root.iterdir() if f.is_file() and f.name.lower().startswith(('license', 'copying', 'notice'))]
    if package.get('license_file'):
        file = root / package['license_file']
        if file.is_file() and file not in files:
            files.append(file)
    for file in sorted(files):
        output.append('\n' + file.name + '\n' + file.read_text(errors='replace'))
    if not files:
        output.append('The source archive declares the SPDX license above but omits its license file. Upstream notice verification remains a formal-release gate.\n')
Path('THIRD_PARTY_NOTICES.txt').write_text('\n'.join(output))
print(f"Inventoried {len(metadata['packages']) - 1} dependencies.")
