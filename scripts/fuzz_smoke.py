"""Deterministic bounded subprocess fuzz smoke; scans text but never runs it."""
import json
import random
import subprocess
import sys
import tempfile
from pathlib import Path
binary = str(Path(sys.argv[1]).resolve())
rng = random.Random(20260918)
with tempfile.TemporaryDirectory() as tmp:
    root = Path(tmp)
    seeds = [b'x=' + b'(' * 300 + b'1' + b')' * 300,
             b'x=' + b'1+' * 5000 + b'1', b'\xef\xbb\xbfimport torch\r\n',
             b'x = f"{1}"\n', b'import torch\nfor i in range(2):\n torch.cuda.synchronize()\n']
    fixtures = Path(__file__).resolve().parents[1] / 'tests/fixtures/preview-rules.json'
    seeds.extend(c['source'].encode() for c in json.loads(fixtures.read_text()))
    alphabet = b'abcxyz0123()[]{}\n\t :;.,+-*=\'"#'
    for _ in range(200):
        seeds.append(bytes(rng.choice(alphabet) for _ in range(rng.randrange(1, 2000))))
    for i, seed in enumerate(seeds):
        (root / 'case.py').write_bytes(seed)
        config = root / 'pyproject.toml'
        if i % 4 == 0:
            config.write_text('[tool.gsu]\nexclude=[' + json.dumps(seed[:40].decode('utf-8', 'replace')) + ']\n')
        elif config.exists():
            config.unlink()
        result = subprocess.run([binary, 'check', '--preview', '--output-format', 'json'], cwd=root, capture_output=True, timeout=5)
        assert result.returncode in (0, 1, 2), (i, result.returncode, result.stderr)
        json.loads(result.stdout)
print(f'Fuzz smoke: {len(seeds)} bounded cases, no crashes or malformed reports.')
