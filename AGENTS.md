# AGENTS.md — GSU

## Project overview

GSU is a standalone, read-only Rust CLI (`gsu`) that statically detects potential PyTorch
performance issues in Python source code. Key product constraints:

- It **never imports or executes** the scanned Python code. It needs no Python, PyTorch,
  CUDA, GPU, network access, or API key at runtime. No telemetry, no plugins, no auto-fix,
  no persistent cache, no editor integration.
- Two subcommands: `gsu check [paths...]` (scan and report) and `gsu rule CODE`
  (print a rule's full explanation).
- 16 rules total: 8 stable/enabled-by-default (T001, T002, S001, S002, D001, D002, P001,
  P002) and 8 **preview** rules (S003–S005, M001–M003, A001, A002) that require
  `preview = true` in config or `--preview` on the CLI. All findings are static evidence
  requiring human review — never measured speedups.
- Exit codes: 0 = clean complete scan, 1 = diagnostics found, 2 = invalid
  arguments/config, incomplete scan, or output failure. Error codes use the `GSU-E0xx`
  scheme.

The current version is **0.2.0, a development candidate** — local Linux checks are
verified, but cross-platform release acceptance and independent review are still pending
(see `docs/release/status.md`).

## Documentation layout

- `README.md` — user-facing behavior: CLI, configuration, rules, analysis boundaries,
  resource limits, exit codes. Keep it in sync with behavior changes.
- `gsu_v0.1.md`, `gsu_v0.2.md` — authoritative product specifications, **written in
  Chinese**. v0.2 is the current spec; v0.1 is the historical baseline.
- `docs/rules/T001.md` … `docs/rules/A002.md` — one Markdown file per rule. These are
  compiled into the binary via `include_str!` in `src/rules/mod.rs`, so editing them
  changes `gsu rule` output.
- `docs/adr/001-parser.md`, `docs/adr/002-preview-and-reports.md` — architecture
  decisions (parser choice, resource limits, preview model, JSON v2 migration).
- `docs/release/status.md` — acceptance record and open release gates.
- `schemas/diagnostics-v2.schema.json` — the JSON report contract (v0.2 emits
  `schema_version: "2"`). `diagnostics-v1.schema.json` is retained only for archived
  v0.1 reports; there is no v1 output mode.

## Technology stack and build

- Rust, **edition 2024, pinned toolchain 1.94.0** (`rust-toolchain.toml`; MSRV is
  deliberately the same verified compiler — no lower MSRV is claimed). `Cargo.lock` is
  committed; always build/test with `--locked`.
- Parsing: `rustpython-parser` / `rustpython-ast` **pinned to =0.4.0**, parser only (no
  RustPython VM), with `default-features = false` and the `num-bigint` backend.
- Other deps: clap 4 (derive), serde/serde_json, toml, globset, ignore, pathdiff,
  unicode-width, libc. Dev-dependency: tempfile.
- Python (3) is used **only for development scripts** under `scripts/` and
  `benches/`; `scripts/validate_schema.py` additionally requires `jsonschema==4.26.0`.
  Rust tests do not need Python. The exception is `python/`, the thin PyPI wrapper
  package (`gsu` console script that `os.execv`s a bundled prebuilt binary); its wheels
  are assembled by `scripts/build_wheels.py` from `dist/` release archives and never
  require building Rust from source.

Build and verify:

```sh
cargo build --release --locked
./target/release/gsu --version
./target/release/gsu check .        # note: examples/train.py intentionally returns 1
```

## Code organization

Single binary crate (`src/main.rs` declares the modules; there is no lib target):

- `src/main.rs` — entry point; clap parsing, panic containment around `check`, exit-code
  mapping.
- `src/cli.rs` — clap definitions: `check`/`Rule` subcommands, `OutputFormat`
  (full/concise/json; `console` is an alias of `full`), `Color` (auto/always/never).
- `src/config.rs` — `[tool.gsu]` TOML config discovery (walks up from cwd for the first
  `pyproject.toml` with `[tool.gsu]`, stopping at a `.git` directory), selector
  resolution (exact codes or categories T/S/D/P/M/A), preview semantics, exclude globs.
  Unknown config keys fail (`deny_unknown_fields`).
- `src/discovery.rs` — file discovery: regular UTF-8 `.py` files only, built-in
  directory exclusions, project-local `.gitignore` support, no symlink following.
- `src/checker.rs` — per-file orchestration: 2 MiB cap, `O_NOFOLLOW` on Unix, UTF-8
  validation, and spawning **one dedicated worker thread per file with a 32 MiB stack**
  to tolerate parser/AST recursion. Analysis is sequential.
- `src/parser.rs` — the **only** parsing adapter around rustpython; pre-parse lexing
  enforces resource limits (500k tokens, nesting depth 256, 4,096 tokens per logical
  statement) and returns GSU-E006/GSU-E008 instead of truncating.
- `src/source.rs` — source text model: BOM handling, line index, Unicode-scalar columns,
  control-character escaping (`safe()`), display-width-aware underlines.
- `src/semantic/` — the lightweight static analyzer (an owned AST visitor, not an
  interpreter; no CFG fixed point, heap model, or interprocedural analysis):
  - `mod.rs` — `Analyzer`, fact environment, loop context, sequence epochs, diagnostic
    emission.
  - `facts.rs` — `Device`, `Tensor`, `Conversion`, `Fact` types.
  - `calls.rs` — normalizes supported PyTorch calls into conversion events.
  - `supported.rs` — finite API signatures; unsupported overloads, extra args, unknown
    keywords, `*args`/`**kwargs` are rejected and stay unknown (deliberately
    conservative).
  - `accumulation.rs` — the bounded syntactic M003 accumulating-cat check.
  - `bindings.rs` — name/shadowing collection.
- `src/rules/mod.rs` — the rule registry: all 16 `RuleMetadata` entries (code, category,
  preview flag, severity, message, short help, suggestion, and `details` via
  `include_str!` from `docs/rules/`). `src/rules/conversion.rs` turns conversion
  sequences into diagnostics.
- `src/diagnostic.rs` — `Diagnostic`, `Evidence`, `RunReport` (schema v2 envelope),
  deterministic sort/dedup in `finish()`, exit-code computation.
- `src/output/mod.rs` — renders full (Ruff-style excerpt + underline), concise, and JSON
  reports; color honors `NO_COLOR` unless `always`.

Resource limits to respect: 2 MiB/file, 500k tokens, depth 256, 4,096 tokens/statement,
100k candidate files, 100k diagnostics.

## Testing strategy

`cargo test --locked` runs everything (267 tests as of 0.2.0). Tests are integration
tests in `tests/` driving the compiled binary via `env!("CARGO_BIN_EXE_gsu")` against
temporary directories, asserting on JSON output:

- `tests/rules.rs` — stable-rule fixtures from `tests/fixtures/rules.json`; each case
  asserts exact count, precise Unicode start/end locations, severity, confidence, and
  non-empty evidence.
- `tests/preview_rules.rs` — same pattern for preview rules
  (`tests/fixtures/preview-rules.json`); every preview rule has ≥12 cases.
- `tests/cli.rs` — CLI/config/selection/ignore/discovery/exit-code behavior.
- `tests/v02.rs` — v0.2 scenario matrices.
- `tests/fixtures/projects/`, `tests/fixtures/preview-projects/` — synthetic
  MIT-licensed training-style sample projects with manifests.
- `tests/snapshots/` — plain-text `full`/`concise` output snapshots.

When adding or changing a rule, update: the registry in `src/rules/mod.rs`, its
`docs/rules/CODE.md`, the JSON fixtures, and possibly the schemas. Fixture locations are
1-based Unicode scalar columns with exclusive end positions.

Additional development-only verification (needs the release binary):

```sh
python3 scripts/validate_schema.py target/release/gsu   # 225 JSON scenarios vs schema v2
python3 scripts/fuzz_smoke.py target/release/gsu        # deterministic bounded fuzz smoke
python3 benches/benchmark.py target/release/gsu --files 100 --runs 30
python3 scripts/package.py <target-triple>              # builds dist archive + checksum
python3 scripts/build_wheels.py [--linux-tag TAG]       # repacks dist archives into dist/wheels/ platform wheels
python3 scripts/notices.py                              # regenerates THIRD_PARTY_NOTICES.txt
```

## Code style and conventions

- Gates (also CI): `cargo fmt --check`, `cargo clippy --locked --all-targets --
  -D warnings`, `cargo test --locked`, `cargo build --release --locked`.
- Style is terse and comment-light; the few comments explain *why* (e.g. the bounded
  analysis stack). Match that; do not add doc boilerplate.
- Conservatism is a core design rule: unknown types/calls/overloads discard facts rather
  than guess; never assume runtime default device/dtype. New checks should trade
  coverage for fewer false positives, and every rule message must make clear that human
  review is required (severity and confidence are separate; high confidence means strong
  static evidence, not measured impact).
- Diagnostics and summaries go to **stdout**; errors and skip notices to **stderr**; JSON
  output is deterministic and never colored.
- User-controlled text (paths, source excerpts) must be escaped via `source::safe`
  before terminal styling.

## Security considerations

- The tool is read-only and offline: no code execution, no network, no writes outside
  requested output, no credential access. Keep it that way — new dependencies or
  capabilities need an ADR.
- Untrusted input hardening already in place: file-size/token/depth/diagnostic caps,
  bounded-stack worker threads, panic containment, `O_NOFOLLOW`, no symlink scans. OOM,
  stack overflow, and hostile-input containment are explicitly *not* fully guaranteed.
- Licenses: MIT; third-party licenses are collected in `THIRD_PARTY_NOTICES.txt`
  (regenerate via `scripts/notices.py`).

## CI and release

- `.github/workflows/ci.yml` — on push/PR, matrix over ubuntu-22.04, macos-14,
  windows-2022: fmt, strict clippy, tests, release build; Linux additionally runs schema
  validation and fuzz smoke; all jobs smoke-test `--version`.
- `.github/workflows/release.yml` — manual/tag-triggered candidate packaging via
  `scripts/package.py` for three targets (x86_64 Linux, aarch64 macOS, x86_64 Windows);
  uploads archives + checksums as artifacts only — it does **not** publish a release.
  The script derives the version from `Cargo.toml` and rejects a binary version
  mismatch.
- Prebuilt archives live in `dist/`; release acceptance status lives in
  `docs/release/status.md`.
- Distribution entry points at the repo root: `.pre-commit-hooks.yaml` (`language:
  system`, expects `gsu` on PATH) and `action.yml` (composite GitHub Action that runs
  the PyPI wheel via pipx). Both depend on the PyPI package being published.
