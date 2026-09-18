# ADR-001: RustPython AST and bounded sequential analysis

Status: accepted for the development candidate; production release acceptance pending.

## Decision

Use `rustpython-parser` 0.4.0 and `rustpython-ast` 0.4.0 (MIT), without RustPython VM. Enable the supported `num-bigint` backend with default features disabled, avoiding the larger default arithmetic backend. The adapter in `src/parser.rs` isolates parsing; native Rust code performs all analysis. The API was checked against the [versioned official documentation](https://docs.rs/rustpython-parser/0.4.0/rustpython_parser/).

Pin Rust 1.94.0, edition 2024, and commit Cargo.lock. The supported minimum compiler is deliberately the same verified compiler, 1.94.0; no lower MSRV is claimed. All transitive versions and checksums are in the lockfile. Third-party licenses are collected in `THIRD_PARTY_NOTICES.txt`.

## Evidence

The local test suite compiles and runs with 1.94.0. Parser fixtures cover match patterns (3.10), exception groups (3.11), generic functions and type aliases (3.12), dictionary expressions within f-strings, invalid syntax, UTF-8 BOM, Unicode columns, and CRLF. This is a tested syntax subset, not a proof of full Python 3.10–3.12 compatibility. Broader PEP 701 variants and full-version compatibility remain release review items.

Byte ranges come from the parser over BOM-stripped text; Source maps them back to the original source using the BOM offset and line index. User-visible columns count Unicode scalar values; BOM is not displayed as a source character.

## Resource choice

Pre-parse lexing enforces 500,000 tokens, structural depth 256, and an additional 4,096-token logical-statement limit. The latter also bounds long left-deep operator and attribute trees. File input is capped at 2 MiB before parsing. These deliberately conservative bounds return GSU-E008 instead of truncating analysis.

Analysis is sequential. Each file uses one joined worker thread with a 32 MiB virtual stack reservation to tolerate parser/AST recursion within limits; no thread pool or parallel file analysis exists. Virtual stack reservation is not a 32 MiB resident-memory allocation. This small architectural adjustment supports safe recursive parser traversal and is included in measured performance. OOM, stack overflow, signals, and hostile-input behavior are not claimed to be fully contained by panic catching.

## Semantics

The owned AST visitor evaluates calls in order and emits pure conversion events to `src/rules/conversion.rs`; other single-point checks share the rule registry. Local facts distinguish import provenance, Tensor identity, device and dtype. Sequence epochs prevent proofs across calls and control-flow boundaries. The analyzer does not implement a Python interpreter, CFG fixed point, heap model, or interprocedural analysis.

The model is intentionally conservative. Unsupported overloads, opaque operations, and escaping values discard facts. Runtime default-device and default-dtype settings are never guessed. Versioned PyTorch documentation for human review: [Tensor.to](https://docs.pytorch.org/docs/stable/generated/torch.Tensor.to.html) and [synchronize](https://docs.pytorch.org/docs/stable/generated/torch.cuda.synchronize.html). No claim is made that every PyTorch release/overload has been verified.

## Release evidence

Actual local build size and benchmark measurements are recorded separately in `docs/release/status.md`. Cross-platform compatibility, cold-cache timings, real-world false-positive review, and hostile-input extended fuzzing must be checked before official distribution.
