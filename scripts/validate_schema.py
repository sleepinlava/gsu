"""Development-only schema and contract checks; never executes scanned Python."""
import json
from pathlib import Path
import subprocess
import sys
import tempfile
from jsonschema import Draft202012Validator

root = Path(__file__).resolve().parents[1]
binary = Path(sys.argv[1]).resolve()
schema = json.loads((root / 'schemas/diagnostics-v2.schema.json').read_text())
Draft202012Validator.check_schema(schema)
validator = Draft202012Validator(schema)
fixtures = json.loads((root / 'tests/fixtures/rules.json').read_text())
fixtures += json.loads((root / 'tests/fixtures/preview-rules.json').read_text())
with tempfile.TemporaryDirectory() as tmp:
    tmp = Path(tmp)
    scenarios = [(c['source'].encode(), None) for c in fixtures]
    scenarios += [(b'def :', None), (b'\xff', None), (b'', '[tool.gsu]\ninvalid=true')]
    for source, config in scenarios:
        (tmp / 'case.py').write_bytes(source)
        if config:
            (tmp / 'pyproject.toml').write_text(config)
        result = subprocess.run([str(binary), 'check', '--preview', '--output-format', 'json'], cwd=tmp, capture_output=True, timeout=10)
        assert not result.stderr, result.stderr
        report = json.loads(result.stdout)
        validator.validate(report)
        summary = report['summary']
        assert summary['diagnostics'] == len(report['diagnostics'])
        assert summary['files_discovered'] == sum(summary[k] for k in ['files_checked', 'files_failed', 'files_skipped'])
        assert report['complete'] == (not report['errors'])
        assert result.returncode == (2 if report['errors'] else int(bool(report['diagnostics'])))
        for diagnostic in report['diagnostics']:
            loc = diagnostic['location']
            assert (loc['start']['line'], loc['start']['column']) <= (loc['end']['line'], loc['end']['column'])
print(f'Validated schema and report invariants for {len(scenarios)} scenarios.')
