---
name: project-migration-state
description: Where the Stylus SDK 0.10.10 / Rust 1.91.0 migration stands: merged work, running agents and their worktrees, shared devnet, next steps.
metadata:
  type: project
---

Repo: /private/tmp/bench/ws, branch `bench` (base v0.3.0 = 47ad800).

Merged on bench (latest first): 3f1be0f ci+scripts (devnet scripts, root Stylus.toml, workflows use cargo-stylus 0.10.10); ac6bde4 devnet assert module; 048c045 rustfmt; e1c47a9 Self::Error -> concrete error in access/finance public impls (export-abi); 35da7ca proxy; 219865f erc20; ea30dcb erc721; de7fa81 utils; b3024bc access-finance; db93eda erc1155; ef2aa6c check-cfg contract-client-gen; 15d3460 fuzz manifest; c111e42 devnet bin; f213b3a devnet helper; 66e5232 alloy aligned to SDK lock (1.6.0 primitives, facade 1.8.3); 7e36233 foundation.

State: contracts lib + tests compile on bench; `cargo nextest run -p openzeppelin-stylus --lib`: 632 run, 632 pass, 3 skipped. clippy -D warnings clean. fmt clean. export-abi lib compiles.

Running agents (background; check their notifications):
- E2E ports on branches agent/e1..e5 (worktrees /tmp/bench/wt/e1..e5, base ac6bde4): e1 erc20 family+erc4626; e2 safe-erc20+erc721 family; e3 erc1155 family; e4 access/finance/proxy examples; e5 benches+templates/crypto-erc20+basic examples. Brief: /tmp/bench/notes/e2e-brief.md.
- Security on agent/sec1 (tokens/access/finance/utils/crypto) and agent/sec2 (proxy/storage/macros/unsafe). Brief: /tmp/bench/notes/security-brief.md. Notes: /tmp/bench/notes/sec1-findings.md, sec2-findings.md.
- Docs on agent/docs (base 3f1be0f). Brief: /tmp/bench/notes/docs-brief.md.
- Audit review done: /tmp/bench/notes/audit-findings.md.

Shared devnet for agent E2E runs: http://localhost:55000 (started from target/debug/devnet; lockf /tmp/bench/e2e.lock serializes runs).

Pending after merges: remove lib/e2e and lib/e2e-proc and their workspace entries; examples must be built to WASM and checked against 24 KiB limit (erc4626 and erc721_metadata were ~24 KB on v0.3.0 — biggest risk); gas before/after with the benches on one clean devnet (BENCH_WASM_DIR for v0.3.0 WASM at /tmp/bench/wt/v030/target/wasm32-unknown-unknown/release vs new); differential tests v0.3.0 vs new on devnet; fuzz targets; coverage 100% target; DX macros (decide scope); upgrade guide (docs page, lead writes); MIGRATION.md and SECURITY-REVIEW.md.

Rules that bind every agent: commit trailer Haiku only; sonnet agents sometimes add a Sonnet trailer: strip it before merging; `Self::Error` in #[public] impls breaks export-abi; TestVM mocks share one return buffer and unmatched calls succeed.

Why: keeps the migration state visible across context windows.
