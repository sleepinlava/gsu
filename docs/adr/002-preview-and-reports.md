# ADR-002: Preview rules, terminal reports and JSON v2

Status: accepted for the 0.2 development candidate (2026-09-18).

The rule catalog now contains eight stable and eight preview rules. Stable defaults remain unchanged; preview rules require an explicit mode switch and are still constrained by selection and ignore settings. Rule categories, status, documentation and short help live in the registry, avoiding separate selector catalogs.

Default full output separates concise actionable advice from the complete explanation retained by `gsu rule` and JSON. Concise output supports log scanning. Standard-library terminal detection and explicit color overrides add no dependency; untrusted source/path text is escaped before ANSI styling. The old `console` spelling aliases full, but its textual layout changes.

The v1 JSON schema used a closed enum of eight rule codes. Rather than silently changing that archived contract, every 0.2 JSON report uses version 2 with an expanded enum and otherwise unchanged fields. Consumers must update their schema and version handling. CLI exit codes and Unicode scalar location semantics are unchanged.

Finite API summaries improve Tensor coverage without guessing model/DataLoader types or executing user code. Views conservatively share identity and break redundant-conversion proofs. Reductions preserve device but not implicit dtype. Calls with unsupported overloads or later argument side effects lose facts. The accumulating-cat rule is bounded to a straight-line syntactic recurrence and does not prove complexity or tensor shapes.

No new production dependency, cache, plugin API, fix model or editor protocol is introduced. Every new rule remains preview during this version; source-pattern confidence and measured runtime impact remain distinct.
