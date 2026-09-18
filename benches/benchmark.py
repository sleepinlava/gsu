"""Reproducible warm-cache benchmark. Does not clear system caches."""
import argparse
import json
import hashlib
import platform
import statistics
import subprocess
import tempfile
import time
import sys
from pathlib import Path
p = argparse.ArgumentParser()
p.add_argument('binary', type=Path)
p.add_argument('--files', type=int, default=100)
p.add_argument('--runs', type=int, default=30)
a = p.parse_args()
binary = str(a.binary.resolve())
block = '''def step_{i}(model, batch, optimizer):
    # Representative untyped training control flow remains conservative.
    optimizer.zero_grad()
    result = model(batch)
    loss = result.sum()
    loss.backward()
    optimizer.step()
    return loss

'''
with tempfile.TemporaryDirectory() as directory:
    root = Path(directory)
    source = 'import torch\n' + ''.join(block.format(i=i) for i in range(112))
    source = ''.join(line.ljust(96) + ' # corpus\n' for line in source.splitlines())
    for i in range(a.files):
        (root / f'train_{i:04}.py').write_text(source)
    def run():
        start = time.perf_counter()
        subprocess.run([binary, 'check', directory, '--output-format', 'json'], stdout=subprocess.DEVNULL, check=True)
        return time.perf_counter() - start
    for _ in range(5):
        run()
    times = sorted(run() for _ in range(a.runs))
    print(json.dumps({'platform': platform.platform(), 'processor': platform.processor(),
        'binary_sha256': hashlib.sha256(Path(binary).read_bytes()).hexdigest(),
        'source_sha256': hashlib.sha256(source.encode()).hexdigest(),
        'peak_child_rss_kib': __import__('resource').getrusage(__import__('resource').RUSAGE_CHILDREN).ru_maxrss if sys.platform.startswith('linux') else None,
        'files': a.files, 'bytes': len(source.encode()) * a.files, 'lines': source.count('\n') * a.files,
        'warmups': 5, 'runs': a.runs, 'median_seconds': statistics.median(times),
        'p95_seconds': times[min(len(times)-1, int(len(times)*.95))],
        'binary_bytes': Path(binary).stat().st_size, 'cache': 'warm; cold cache unverified'}, indent=2))
