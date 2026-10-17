# Stylus SDK 0.10.10 migration

This migration moves the v0.3.0 codebase at `47ad80064cf37be08ae714257695ba281e5471ad` to library version 0.4.0, Stylus SDK 0.10.10, stable Rust 1.91.0, and Alloy 1.7.3. The workspace, all contract crates, macros, crypto library, examples, template, fuzz targets, benches, tests, CI, and docs were included.

## Environment

The required environment was present before work began. Docker was running.

| Tool | Version |
| --- | --- |
| `rustc` / `cargo` | 1.91.0 |
| `cargo-stylus` | 0.10.10 |
| `stylus-sdk`, `stylus-tools`, `stylus-test` | 0.10.10 |
| Docker | 29.2.1 |
| Binaryen `wasm-opt` | 116 |
| `cargo-fuzz` | 0.13.2 |
| `cargo-llvm-cov` | 0.9.1 |
| `cargo-audit` | 0.22.2 |
| `cargo-deny` | 0.20.2 |
| Foundry `cast` | 1.4.4-stable |
| Node / npm | 24.6.0 / 11.5.1 |
| `pdftotext` | 26.10.0 |
| Python | 3.13.7 |
| Git | 2.54.0 |

## SDK changes adopted

I reviewed the [Stylus SDK 0.10.10 changelog](https://github.com/OffchainLabs/stylus-sdk-rs/blob/v0.10.10/CHANGELOG.md) against the 0.9-era API and adopted changes that affect composition, storage, calls, tests, or deployability:

* Replaced global `msg`, `block`, and `contract` accessors with instance VM accessors. Methods that use host state now carry the required host-access context.
* Replaced inheritance-style composition with explicit storage fields, `#[public]`, and `#[implements(...)]`. Interface implementation and the exposed public router are now explicit at each contract.
* Updated generated cross-contract clients for Alloy 1.7 and SDK call contexts. Read-only operations use `Call::new()`; mutating operations use `Call::new_mutating(self)`. VM access is passed to the client.
* Adopted high-level SDK calls where their return-data semantics fit. High-level `call`, `delegate_call`, and `static_call` persist the storage cache before transferring control. Raw calls still need explicit cache handling; SafeERC20 clears dirty storage before a raw call that can reenter.
* Adopted the SDK's canonical ABI validation paths for untrusted return data and updated `MethodError`, storage integer update, `StorageU96`, and EIP-712 APIs.
* Replaced Motsu storage tests with a fresh SDK `TestVM` per unit test. Replaced `lib/e2e` and `lib/e2e-proc` with the SDK-managed Nitro devnet from `stylus-tools`.
* Added `Stylus.toml` markers to deployable packages. The root pins Binaryen 116 and `--enable-bulk-memory -O3`, so build, checking, and gas results use the same optimizer recipe. WASM builds use stable Rust's `wasm32-unknown-unknown` target.
* Forwarded the SDK's still-available `reentrant` feature for callback-heavy contracts. The guard is deprecated upstream and remains enabled unless the feature is enabled; the flash-mint example enables it because borrower callbacks can call the lender.
* Kept `ruint` default features disabled for `no_std` WASM builds and aligned direct Alloy crates to 1.7.3. The template has a matching lockfile and toolchain.
* Added per-`#[public]` and per-`#[interface_id]` selector collision checks in the local SDK proc-macro patch. These checks catch collisions within an annotated block/trait; they do not prove uniqueness across separate public blocks or that a custom selector on an interface trait matches every implementation.

The implementation choices also follow the explicit-storage and capability-oriented patterns used by OpenZeppelin's [Solidity](https://docs.openzeppelin.com/contracts/5.x/), [Cairo](https://github.com/OpenZeppelin/cairo-contracts), [Stellar](https://github.com/OpenZeppelin/stellar-contracts), and [Sui Move](https://github.com/OpenZeppelin/sui-move-contracts) libraries: state and policy remain separate, privileged behavior is explicit, and composable extensions expose reusable operations instead of trying to emulate Solidity inheritance.

## Extension and test design changes

* `Capped::check_increase(total_supply, amount)` is a pure reusable policy check. It returns the standard cap error without writing storage. A capped flash-mint contract must call `max_flash_loan_with_cap` and `flash_loan_with_cap`; generic flash-mint helpers cannot discover a sibling cap extension.
* ERC-721 and ERC-1155 operations validate their fallible balance/ownership conditions before their first write where possible. ERC-1155 batch validation aggregates repeated token IDs, which remain valid input.
* ERC-1967 upgrade helpers validate target code first, preserve v0.3 error precedence, restore the previous implementation/beacon slot when setup fails, and emit upgrade events only after successful setup.
* E2E tests use one shared support crate for SDK devnet accounts, deployment/activation, receipts, events, and reverts. The old bespoke node script and E2E proc macro were removed.
* New tests target failure behavior that ordinary ABI reverts can hide: Rust `Err` values do not automatically revert a caller's frame if a composing contract catches them.

## Compatibility and upgrade guide

The detailed, actionable guide is [docs/modules/ROOT/pages/upgrade.adoc](docs/modules/ROOT/pages/upgrade.adoc). Its breaking changes and required actions are:

* Pin Rust 1.91.0, SDK/cargo-stylus 0.10.10, and Alloy 1.7.3. Add workspace `Stylus.toml` plus a `[contract]` marker for each deployable crate.
* Replace global accessors, old generated-client calls, `#[inherit]`, old `MethodError` imports, old EIP-712 hooks, and old storage arithmetic helpers with the VM/call-context/trait APIs described above.
* Remove `motsu`, `e2e`, and `e2e-proc` downstream dependencies. Use `TestVM` for local storage tests and SDK devnet tooling for cross-contract and activation tests. `TestVM` instances are separate hosts; they do not stand in for deployed external contracts.
* The canonical ERC-721 receiver selector is now `onERC721Received`. The v0.3 wrapper example accidentally exposed `on_erc721_received`; integrations using that noncanonical selector must update.
* Upgrade events keep their signatures but now follow a successful initializer. Initializer logs therefore precede `Upgraded` / `BeaconUpgraded`; indexers that assumed the former order need to adjust.
* UUPS initialization now uses a namespaced one-time marker and checks legacy initialized state. Existing proxy layouts do not move, but downstream initializers must call the helper and define a legacy-state check appropriate to their authorization model. The example's `setVersion` is owner-only and rejects replay.
* `Erc721Enumerable` adds an unstructured presence marker so token ID zero can be indexed. Existing ID-zero ownership needs a reviewed backfill before enumeration can be trusted.
* Existing `#[storage]` field order/types and EIP-1967 slots were not changed. `BeaconProxy` keeps its previous ordinary field reserved and reads the active beacon from the EIP-1967 beacon slot. v0.3 construction could overwrite ordinary slot zero after delegate initialization; the migration cannot recover an application value already overwritten there.
* Adding `AccessControlEnumerable`, `Erc721Enumerable`, `Erc1155Supply`, or `Capped` to a live proxy adds semantic state that is not reconstructed by layout compatibility. The upgrade guide requires an explicit trusted-data backfill. `Erc721Enumerable` and `Erc721Consecutive` remain incompatible.
* `Erc721Wrapper` rejects no-code underlying assets and unbacked receiver callbacks. `Erc20Wrapper::_recover` now identifies the configured underlying token in its error. EdDSA callers must supply canonical field/scalar encodings. Invalid Poseidon2 exponents/round sets now fail construction.
* Compose extensions using the documented policy helpers and propagate `Err` with `?` when the public call must be atomic. Catching an extension error can preserve earlier writes, logs, or successful external calls.

A source comparison of every production `#[storage]` field between v0.3.0 and this tree found no field name, type, or order changes. The new UUPS and ERC-721 enumerable markers use namespaced slots and are called out above.

## Gas measurements

The command `./scripts/bench.sh` built and optimized every contract, then ran the existing serial Nitro devnet benchmark suites. The comparison uses matching `WASM Opt & Cached` rows from the v0.3.0 checkout and the current tree: 64 function rows, with the 21,000 EVM base gas fee omitted by the runner.

| Metric | v0.3.0 → migrated |
| --- | ---: |
| Median absolute change | +240 gas |
| Median relative change | +0.60% |
| Mean absolute change | +584 gas |
| Mean relative change | +1.19% |
| Functions higher / lower | 50 / 14 |

Largest increases were ERC-721 `safeTransferFrom` (+3,965), ERC-1155 `safeBatchTransferFrom` (+3,613), ERC-721 `transferFrom` (+2,730), and ERC-1155 `burnBatch` (+2,669). The largest reductions were Pedersen `hash` (-2,911), Merkle `verify` (-274), and VestingWallet `vestedAmount(uint64)` (-258). The ERC-721/ERC-1155 increases include additional validation needed to prevent partial state changes when composed contracts catch returned errors.

## Optimized WASM sizes

Binaryen 116 produced 32 contract example artifacts and one crypto ERC20 template artifact. All artifacts passed the SDK's devnet checking/activation path in the E2E suite. The current largest artifact is 101,825 bytes, below the 2,000,000-byte per-artifact limit.

| Artifacts | v0.3.0 total | Migrated total | Change | Largest, old → new |
| --- | ---: | ---: | ---: | ---: |
| 32 contract examples | 1,783,340 B | 1,857,118 B | +73,778 B (+4.14%) | 96,966 B → 101,825 B |
| Crypto ERC20 template | 85,879 B | 89,857 B | +3,978 B (+4.63%) | 85,879 B → 89,857 B |
| All 33 artifacts | 1,869,219 B | 1,946,975 B | +77,756 B (+4.16%) | 96,966 B → 101,825 B |

The smallest contract artifact is 17,583 bytes (16,555 bytes at v0.3.0). Size was measured after the configured optimizer, and it was not used as an optimization goal beyond the deployment limit.

## Evidence and check results

| Check | Result |
| --- | --- |
| `cargo +1.91.0 test --locked --workspace --all-targets --features openzeppelin-crypto/ruint` | Passed: 125 crypto tests, 593 contract tests, 5 caught-error regressions, cap, slot-collision, macro, test-support, and legacy-UUPS tests. Devnet integration tests are feature-gated and were run separately. |
| `./scripts/e2e-tests.sh` | Passed across all example/template targets and success/failure scenarios. One initial ERC-721 case was interrupted when Nitro's Docker container exited during startup; the exact case passed on focused retry, and the remaining suite passed on resume. |
| `./scripts/bench.sh` | Passed after pinning the runner to `--bin benches`; every benchmark suite completed. |
| `cargo +1.91.0 build --locked --workspace --exclude benches --exclude stylus-test-support --exclude basic-script-example --release --target wasm32-unknown-unknown` | Passed for all contract examples. |
| `cargo +1.91.0 build --locked --manifest-path templates/crypto-erc20/Cargo.toml --release --target wasm32-unknown-unknown --target-dir target` | Passed for the crypto ERC20 template. |
| Binaryen 116 optimization and artifact size assertion | Passed for all 33 artifacts; total 1,946,975 bytes, maximum 101,825 bytes. |
| `cargo +1.91.0 clippy --locked --workspace --all-targets --all-features -- -D warnings -D clippy::all -D clippy::pedantic` | Passed. |
| `cargo +1.91.0 fmt --all -- --check` and template, fuzz, vendored SDK `fmt --check` | Passed for workspace, `templates/crypto-erc20`, `fuzz`, and `vendor/stylus-proc`. |
| `cargo +1.91.0 doc --locked --workspace --all-features --no-deps` | Passed with no broken intra-doc links or rustdoc warnings. |
| `npm run docs` | Passed; Antora generated `docs/build/site`. |
| `bash -n` for bench, E2E, WASM, and ABI scripts | Passed. |
| Global accessor / Motsu source-and-manifest scan | No active-code or manifest matches. Historical changelog and migration docs explain their removal. |
| `cargo +1.91.0 llvm-cov --locked --workspace --all-targets --features openzeppelin-crypto/ruint --summary-only` | Passed: 90.32% lines, 77.11% branches, 88.29% regions for the workspace. |
| Comparable v0.3.0 core coverage (`openzeppelin-stylus`, `openzeppelin-crypto`, `openzeppelin-stylus-proc`) | Lines 85.89% → 91.72%; branches 77.47% → 79.66%; regions 84.74% → 89.89%. Core coverage increased by 5.83 percentage points in lines. Unit test counts increased from 575 contract / 113 crypto tests to 593 / 125. |
| `cargo +1.91.0 fuzz run merkle --sanitizer none -- -max_total_time=15` | Passed: 2,090,464 runs, 101 coverage counters, 376 features. |
| `cargo +1.91.0 fuzz run keccak --sanitizer none -- -max_total_time=15` | Passed: 10,676,024 runs, 35 coverage counters, 79 features. |
| `cargo run --locked -p benches --bin differential_v03 -- OLD.wasm NEW.wasm` | Passed on the SDK devnet: metadata, final supply/balances/allowance, event topics/data, cap/zero-receiver errors, and paused-transfer errors match v0.3.0. |

The E2E inventory comparison found 399 named v0.3.0 scenarios and 412 in the migrated tree. Each old scenario remains represented; the additional cases cover SDK activation, new failure regressions, and split scenarios. The old UUPS `setVersion` replay-success behavior was intentionally changed to reject replay, and is now tested as a failure because replay can corrupt initialization state.

Coverage did not reach 100%. The broad workspace run reports 90.32%; the comparable contract/crypto/macro packages are 91.72% line-covered. An all-features coverage attempt tried to run devnet tests under LLVM instrumentation, but SDK child WASM builds inherited `-C instrument-coverage` and failed because the Rust toolchain had no WASM `profiler_builtins` runtime. The devnet tests themselves pass in the normal E2E runner. Nightly cargo-fuzz with AddressSanitizer also could not compile the pinned `branches` dependency against nightly 1.92's changed `prefetch_read_data` signature; the stable libFuzzer runs above completed without ASan. No mutation testing or formal verification was run.

`cargo audit` reported five transitive vulnerabilities and ten unmaintained/unsound warnings in the SDK's host-side devnet/test stack; see [SECURITY-REVIEW.md](SECURITY-REVIEW.md). It did not report a contract/WASM dependency vulnerability. `cargo deny check` has no repository `deny.toml` to load and, using its default empty allow-list, rejected MIT/Apache licenses and repeated transitive advisory entries. No allow-list was added to hide audit results.

The SDK build emits missing-documentation warnings at several `#[public]` macro invocations, despite successful strict Clippy and Rustdoc generation. These are proc-macro expansion diagnostics, not undocumented external ABI changes. The migration guide and module pages describe the public composition APIs.
