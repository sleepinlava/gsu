# Development candidate acceptance record

GSU 0.1.0 has been implemented locally from `gsu_v0.1.md`. This record distinguishes completed engineering checks from release requirements that need other platforms or independent review. No official release or registry publication has been performed.

## Verified locally

- Rust 1.94.0; Cargo.lock; standalone binary; all eight rule identifiers.
- Native AST parsing, scope/alias handling, conservative conversions and sequence barriers.
- Console/JSON, strict configuration, file filters, errors and exit codes 0/1/2.
- `cargo fmt --check`, strict Clippy, and all 121 Rust tests pass (96 rule tests plus 25 integration tests).
- 96 independent rule fixtures (12 per rule), with exact start/end positions, severity, confidence and counts.
- Integration tests cover configuration, partial failure, parser syntax, Unicode/BOM/CRLF, links, controls, limits, method-name false positives, scopes and non-execution.
- 20 original MIT-licensed realistic source samples, with expected findings and qualitative review notes in `tests/fixtures/projects/manifest.json`. These are synthetic review examples, not an external project precision study.
- JSON Schema validation and report invariants: 99 scenarios.
- Deterministic malformed-source/configuration smoke: 205 cases, no crash or malformed report.
- Local release build: 5,897,952 bytes. Candidate archive checksum verified after extraction; version and exit codes 0/1/2 pass with `PATH=/nonexistent`.
- Linux syscall trace shows one analysis thread, no scanner-initiated subprocess execution, and no network system calls.
- Packaging scripts, build-source hashes, third-party dependency inventory, and candidate archive with SHA-256 checksum.

## Performance evidence

Environment: Linux x86_64, AMD Ryzen 7 8845H, glibc 2.44, Rust 1.94.0. Reproducible synthetic corpus includes function scopes, model/optimizer calls, and comment padding to approximately 100 KiB per 1,009-line file. Corpus code and generator are MIT-licensed in this repository.

The recorded 100-file / 10.7 MB run has p95 0.529 s; the 1,000-file / 107 MB run has p95 5.327 s and observed peak child RSS 21,280 KiB. RSS is a development-driver child-process high-water mark, not an isolated clean-machine memory certification. These recorded runs use the hash embedded in their reports, immediately before the final explicit-hidden-root discovery fix; the final binary was subsequently rebuilt and package-smoked.

Raw warm-cache results are in `benchmark-100.json` and `benchmark-1000.json`. Each uses five warmups and thirty measured end-to-end scans, including discovery and serialization. These measurements apply only to this local environment and corpus. They are not a PyTorch workload speedup. Cold-cache performance and a stable dedicated-machine regression baseline remain unverified.

## Gates still required before formal release

- Execute the checked-in Linux/macOS/Windows workflows on their actual runners; validate downloaded packages in clean machines without Python/CUDA.
- Establish and test the Linux glibc compatibility baseline; this machine's local binary is not a portable Linux baseline build.
- Independently review rule wording and the real-world false-positive/coverage sample set. Do not advertise an externally validated precision percentage from synthetic examples.
- Run extended coverage-guided fuzzing and dependency security auditing with pinned audit tools. The checked-in bounded smoke is not an extended fuzz campaign.
- Verify upstream copyright/license notices for crates whose published archives omit license text, and review final distribution contents.
- Expand Python 3.10–3.12 grammar coverage, especially PEP 701 f-string variants. Current fixtures demonstrate a subset, not complete version conformance.
- Complete cold-cache benchmarks, clean-platform memory measurements, build provenance/signing, maintainer acceptance, and post-publication download checks.

## Conservative implementation boundaries

Unsupported calls/overloads and dynamic objects are not inferred. No model/DataLoader return typing, heap analysis, cross-file resolution or interprocedural execution is attempted. A 4,096-token logical-statement limit supplements the design's file/token/nesting limits. File analysis uses sequential joined bounded-stack threads, as documented in ADR-001. Linux refuses final-component source symlinks when opening files; exhaustive adversarial filesystem-race containment is not claimed.

## Reproduce

Run the commands in README, then `python3 scripts/package.py x86_64-unknown-linux-gnu` on a matching Linux host. Candidate packaging only creates local files; it does not publish. The release workflow uploads reviewable CI artifacts and deliberately leaves official publication to maintainers after acceptance.
