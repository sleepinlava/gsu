# GSU 0.2.0 development candidate acceptance record

Local Linux verification completed on 2026-09-18. This is a development candidate; nothing has been published. The [v0.1 record](v01-status.md) is preserved separately.

## Verified locally

- Fixed Rust 1.94.0 toolchain, locked dependencies, `cargo fmt --check`, strict Clippy and release build.
- All **267 Rust tests** pass: 96 stable rule cases, 126 preview rule cases, 25 original integration tests and 20 v0.2 integration tests (including larger scenario matrices).
- Each preview fixture independently specifies count, exact Unicode scalar start/end positions, severity and confidence. Every new rule has at least 12 cases.
- Full/concise plain and colored snapshots; console alias; preview/config/selection/ignore; Unicode/BOM/CRLF/tab and multi-line rendering; deterministic partial failures; conservative API/alias/argument-effect regressions.
- Original 20 project samples plus ten new original MIT-licensed training-style samples and qualitative review notes. These are synthetic examples, not external-project precision measurements.
- JSON v2 schema and envelope invariants: **225 scenarios**, including partial/invalid-input reports.
- Preview-enabled deterministic fuzz smoke: **331 inputs**, no crashes or malformed JSON. This is bounded smoke, not an extended fuzz campaign.
- Version 0.2.0 Linux candidate archive, checksum, embedded schema, binary provenance and standalone exit codes 0/1/2 verified after extraction with `PATH=/nonexistent`.
- Candidate package generation derives its version from Cargo.toml and rejects a binary version mismatch. CI uploads only the current package/checksum, avoiding stale archives.

## Same-machine performance comparison

The existing warm-cache 100-file corpus (10.7 MB, 100,900 lines) was scanned with five warmups and thirty measured runs per binary, sequentially on this Linux x86_64 host. The corpus hashes match. Reports include final binary hashes; no PyTorch runtime or GPU workload was measured.

| Measure | v0.1 baseline | v0.2 | Change |
| --- | --- | --- | --- |
| Median | 0.519 s | 0.528 s | +1.8% |
| p95 | 0.535 s | 0.540 s | +0.9% |
| Observed peak child RSS | 21,772 KiB | 21,552 KiB | -1.0% |
| Binary size | 5,897,952 bytes | 6,064,592 bytes | |

Neither time nor observed memory regressed beyond the 15% review threshold. Raw results: [baseline](v02-baseline-100.json), [v0.2](v02-benchmark-100.json). The baseline is the retained v0.1 candidate executable; its prior acceptance record remains historical. This corpus exercises conservative untyped training code with the default rule selection. It is not a worst-case preview-rule benchmark. RSS is the development driver's child-process high-water mark; cold-cache and clean-machine memory certification remain pending.

## Compatibility and remaining formal-release gates

- Default rules remain the original eight; all eight new rules stay preview. Semantic improvements can expose additional existing-rule findings through supported operators.
- Default console layout changes to full, with console as an alias; concise is available for one-line logs.
- Every JSON envelope now declares schema version 2. Consumers must update version handling and schema; field layout, path/column semantics and exit codes are unchanged. The v1 schema is preserved for archived reports.
- Actual macOS/Windows runners, clean-machine package checks, portable Linux glibc baseline, independent rule/real-world review, extended fuzzing, dependency auditing, remaining license review, signing and maintainer acceptance remain open as in the v0.1 release record.
- The binary was built locally on glibc 2.44 and is not advertised as a portable Linux baseline build. No new cross-platform or Python grammar conformance claim is made.

## Reproduce

Use README development commands with Rust 1.94.0. This workstation's existing toolchain is under `/tmp/gsu-cargo` and `/tmp/gsu-rustup`; normal installations can use `cargo` directly. Source and tests do not depend on these temporary paths.
