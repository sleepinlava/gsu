# GSU

GSU is a standalone, read-only Rust CLI that finds potential PyTorch performance issues in Python source. It never imports or executes your project, and does not require Python, PyTorch, CUDA, a GPU, or an API key at runtime.

Version 0.2 follows [`gsu_v0.2.md`](gsu_v0.2.md); [`gsu_v0.1.md`](gsu_v0.1.md) remains the historical baseline. It is a development candidate: local Linux checks are verified; cross-platform release acceptance and independent real-world review remain pending. See [release status](docs/release/status.md).

## Build and use

Install Rust with rustup, then build with the pinned toolchain:

```sh
cargo build --release --locked
./target/release/gsu --version
./target/release/gsu check .
./target/release/gsu rule T001
./target/release/gsu check src/train.py --select T,S
./target/release/gsu check . --output-format concise
./target/release/gsu check . --preview --select S,M,A
./target/release/gsu check . --output-format json > gsu-report.json
```

To install the binary on your PATH: `cargo install --path . --locked`. The resulting `gsu` binary runs independently of Rust and Python installations.

The repository includes `examples/train.py`; checking it intentionally returns 1. All suggestions require human review and validation of correctness, numerical behavior, and actual performance. Findings are static evidence, not measurements or guaranteed speedups.

## Rules

| Rule | Name | What to review |
| --- | --- | --- |
| [T001](docs/rules/T001.md) | transfer-in-loop | Potential device transfers in loops |
| [T002](docs/rules/T002.md) | device-ping-pong | Visible CPU/CUDA round trips without intervening use |
| [S001](docs/rules/S001.md) | item-in-loop | Tensor scalar reads in loops |
| [S002](docs/rules/S002.md) | explicit-sync | Explicit CUDA device synchronization |
| [D001](docs/rules/D001.md) | hardcoded-device | Fixed CUDA indices |
| [D002](docs/rules/D002.md) | repeated-device-cast | Repeated equivalent conversions to a known device |
| [P001](docs/rules/P001.md) | explicit-float64 | Explicit FP64 precision |
| [P002](docs/rules/P002.md) | repeated-dtype-cast | Potential dtype conversions in loops |

The eight rules above remain stable and enabled by default. The following rules require preview:

| Rule | Name | What to review |
| --- | --- | --- |
| [S003](docs/rules/S003.md) | tolist-in-loop | Tensor-to-Python conversion in loops |
| [S004](docs/rules/S004.md) | scalar-conversion-in-loop | `bool/int/float(Tensor)` in loops |
| [S005](docs/rules/S005.md) | nonzero-in-loop | Known CUDA nonzero synchronization in loops |
| [M001](docs/rules/M001.md) | empty-cache-in-loop | Repeated CUDA cache clearing |
| [M002](docs/rules/M002.md) | tensor-copy-in-loop | `torch.tensor(existing_tensor)` copies in loops |
| [M003](docs/rules/M003.md) | accumulating-cat-in-loop | Direct accumulating concatenation in loops |
| [A001](docs/rules/A001.md) | retain-graph-in-loop | Explicit graph retention without higher-order differentiation |
| [A002](docs/rules/A002.md) | anomaly-detection-enabled | Explicit autograd debugging overhead |

Severity and confidence are separate. High confidence means strong static evidence, not a measured performance impact. Low-severity diagnostics also return exit code 1.

## Configuration

```toml
[tool.gsu]
select = ["T", "S", "D", "P"]
ignore = ["D001"]
exclude = ["generated", "**/vendor/**"]
```

GSU searches from the working directory upward for the first `pyproject.toml` containing `[tool.gsu]`, stopping after checking a directory containing `.git`. That directory becomes the project root; otherwise the working directory is used. `--config FILE` chooses one explicit TOML file and its parent as root. Nested configurations are not loaded.

Each CLI field replaces its configuration field. `--select D` does not clear configured ignores; use `--ignore ''` to clear them. `--select ''` or `select = []` disables all rules but still checks source syntax. Selectors accept exact uppercase rule codes or T/S/D/P/M/A. Unknown keys, selectors, and invalid globs fail explicitly.

Preview is off by default. Set `preview = true` in `[tool.gsu]` or pass `--preview`; `--no-preview` overrides the configuration. Without an explicit selection, preview enables all 16 rules. An explicit `select` still limits categories, so `select = ["T", "S", "D", "P"]` does not select M/A rules. Exact preview codes require preview even if also ignored; category selectors skip preview rules when preview is off. Ignore entries can mention preview rules without enabling them. `gsu rule CODE` documents stable/preview status regardless of the current selection.

`--exclude PATTERN` is repeatable and replaces configured excludes. Literal names without `/`, such as `generated`, exclude matching path components. Glob patterns match case-sensitive root-relative paths using `/`. Negation and brace expansion are unsupported. Built-in exclusions always apply.

## Files and reports

Only regular `.py` files encoded as UTF-8 (with optional BOM) are analyzed. Explicit files obey the same filters as discovered files. Hidden directories and common environment/build/cache directories are skipped. Project-local `.gitignore` files work even outside Git repositories. Global ignores, `.ignore`, and `.git/info/exclude` are not used. Symbolic links are not scanned.

The default `full` report shows the rule code, source location, a marked source excerpt, a short help line, severity, confidence and preview status. Multi-line spans indicate their end location. Use `gsu rule CODE` for the complete explanation. `--output-format concise` prints one line per diagnostic plus a summary; `console` is a compatibility alias for `full`.

`--color auto|always|never` controls terminal styling. Auto colors only a terminal and disables color when `NO_COLOR` is present (including empty). Explicit `always` overrides `NO_COLOR`. JSON is always plain text. Source and path control characters are escaped before styling.

Console diagnostics and summary go to stdout; errors and skip notices go to stderr. JSON writes one deterministic object to stdout, including errors and skipped files. The schema is [`schemas/diagnostics-v2.schema.json`](schemas/diagnostics-v2.schema.json). Paths are relative to the project root with `/`; line and Unicode scalar columns are 1-based with exclusive end positions.

**JSON migration:** v0.2 emits `schema_version: "2"` for all JSON reports, including configuration errors. Field names and location semantics are unchanged. Version 1's closed rule enum cannot validate the new codes; consumers must accept version 2 and use the new schema. The original v1 schema is retained for archived v0.1 reports. No v1 output mode is provided.

| Exit | Meaning |
| --- | --- |
| 0 | Complete scan, no selected diagnostics |
| 1 | Complete scan, selected diagnostics found |
| 2 | Invalid arguments/configuration, incomplete scan, or output failure |

A failing file does not prevent other files from being checked. After valid JSON-mode arguments, configuration and scan failures use the same JSON envelope with `complete=false`. Invalid CLI syntax may instead print a text error to stderr. Failed output cannot guarantee a complete JSON document.

## Analysis boundaries

GSU recognizes standard torch imports and aliases, supported Tensor factories, annotated Tensor parameters, and a finite set of conversions. It handles local shadowing, branches, loop contexts, and conservative sequence barriers. Unknown model outputs, DataLoader batches, arbitrary container elements, dynamic imports, and cross-file types are not inferred. Module conversions are not treated as Tensor conversions. Unreachable source can still produce findings.

Supported `empty_like/zeros_like/ones_like/full_like` factories inherit known input device/dtype, subject to explicit overrides. Supported Tensor `view/reshape/flatten/transpose/permute/contiguous` calls preserve device/dtype; shape-changing calls require known integer dimensions (including literal integer lists/tuples). The dtype overload of `view` is excluded. `sum/mean` preserve device and only retain dtype when explicitly recognized. All these operations stop conversion-sequence proofs. Views conservatively share alias identity so unknown mutation invalidates related facts.

M003 only recognizes a direct assignment from `torch.cat` with a literal list/tuple of names containing its outside Tensor accumulator once. Other accumulator writes/uses, aliases, visible escapes, callbacks capturing the accumulator and nested control flow exclude the pattern. It does not infer the other parts' shapes or prove runtime complexity. Dynamic rebinding of module attributes and arbitrary heap/global effects remain outside the analysis model.

Omitted ordinary factory device/dtype arguments are unknown: global defaults may change. Current-device `cuda` is distinct from fixed `cuda:0`. Unsupported or ambiguous conversion overloads lose facts. Unknown calls and intermediate consumption stop redundancy proofs. These boundaries intentionally trade coverage for fewer false positives.

Limits: 2 MiB per source file, 500,000 tokens, structural nesting 256, 4,096 tokens per logical statement, 100,000 candidate files, and 100,000 diagnostics. Resource errors return 2. Processing is sequential using a dedicated bounded-stack analysis thread per file; no persistent cache is created.

No auto-fix, runtime profiling, network access, telemetry, plugins, or environment credentials are used by the CLI.

## Development

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --release --locked
python3 scripts/validate_schema.py target/release/gsu
python3 scripts/fuzz_smoke.py target/release/gsu
python3 benches/benchmark.py target/release/gsu --files 100 --runs 30
```

Python is used only by optional development scripts. Schema validation requires `jsonschema==4.26.0`; Rust tests need no Python. See [ADR-001](docs/adr/001-parser.md) for toolchain, parser, syntax validation, and resource decisions.
