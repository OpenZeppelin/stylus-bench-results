---
name: feedback-working-style
description: "How the user wants this migration run: autonomy, commit cadence, attribution lines, and the priority order for design choices."
metadata:
  node_type: memory
  type: feedback
  originSessionId: be2a1787-77d5-4009-9c42-dfab03703dd8
  modified: 2026-10-19T06:01:25.045Z
---

- Work autonomously: do not ask questions mid-task; pick the best option, write down why, move on.
- Environment check first: list tools with versions; stop only if something needed is missing; never install anything.
- Commit in small logical steps on the working branch. Commit trailer: `Co-Authored-By: Claude Haiku 5.5 <noreply@anthropic.com>`. PR descriptions end with the "Generated with Claude Code" line.
- Priorities for every design/refactor choice, in order: (1) developer experience of composing contracts and extensions, (2) gas (measure with benches before/after), (3) WASM size (must fit the limit; otherwise not a goal).
- Trust tool output only: a build, a test, a fuzzer. Do not judge own work without evidence.
- Do not comment out code or tests; do not drop E2E scenarios (success and failure) from v0.3.0.
- Security: for each new bug commit a failing PoC test first, then the fix in a separate commit. Do not trust audits/ resolved claims without checking code.

Why: the user gave a very large, autonomous brief and wants verifiable results, not claims.
How to apply: when facing a choice, decide, record the reason in MIGRATION.md, continue.
