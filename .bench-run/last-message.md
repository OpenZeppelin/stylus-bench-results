The migration is committed and the working tree is clean. The final clean run passed **884 tests across 118 test binaries**. Strict workspace Clippy, rustdoc tests, documentation generation, formatting, and the legacy API scan also passed.

- All 496 retained v0.3.0 E2E scenarios passed, including failure cases. All 33 example and template WASM configurations activated on the SDK devnet.
- Across 49 matched benchmark operations, mean gas changed by **−0.60%**. Maximum raw WASM size increased from 125,487 to 128,086 bytes; the largest compressed artifact was 34,861 bytes and fit in two fragments.
- Coverage reached **98.18% of lines, 98.18% of regions, and 96.71% of functions**, above the v0.3.0 baseline. The 100% target was not reached; the reports describe remaining instrumentation gaps and unreachable paths.
- Fuzzing completed without crashes, but macOS AddressSanitizer stalled before target initialization, so those runs used no sanitizer.

The security review records findings C-01 through C-19, their PoCs and fixes, and the audit cross-check. It also identifies open review items C-05, C-07, and C-08, plus the documented exact-transfer asset constraint.

- [Migration report](/tmp/bench/ws/MIGRATION.md)
- [Security review](/tmp/bench/ws/SECURITY-REVIEW.md)
- [Upgrade guide](/tmp/bench/ws/docs/modules/ROOT/pages/upgrading.adoc)