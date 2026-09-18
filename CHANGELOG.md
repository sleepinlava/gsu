# Changelog

## 0.2.0 — development candidate

- Add opt-in S003–S005, M001–M003 and A001–A002 performance checks, with per-rule boundaries and manual suggestions.
- Add preview configuration and CLI overrides; preserve the original eight default rules.
- Add Ruff-style full/concise reports, console alias, terminal colors and short help; full explanations remain in JSON and rule help.
- Extend Tensor facts through four like factories, supported shape/layout methods and sum/mean. Preserve conservative aliases and invalidate stale argument facts.
- Emit JSON schema version 2 with the expanded rule catalog; retain the historical v1 schema.
- Add 126 preview rule fixtures, output snapshots, semantic/CLI regressions and ten reviewed original training-style samples.
- Derive candidate package versions from Cargo.toml and reject stale binaries.
- No automatic fixes, inline ignores, editor integration or runtime execution. Formal release gates remain open.

## 0.1.0 — development candidate

- Add standalone Rust `check` and `rule` commands.
- Add all eight frozen rules with conservative Tensor/device/dtype facts.
- Add TOML configuration, deterministic discovery, console and schema-versioned JSON.
- Add parser/resource guards, regression fixtures, CI, and release packaging workflow.
- Formal release acceptance remains tracked in `docs/release/status.md`.
