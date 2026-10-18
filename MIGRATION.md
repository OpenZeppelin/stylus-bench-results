# Migration to Stylus SDK 0.10.10

## Summary

The workspace now targets stable Rust 1.91.0 and Stylus SDK/Cargo Stylus
0.10.10. Contracts, extensions, proc macros, crypto, examples, benchmarks,
fuzz targets, docs, and deployment scripts were migrated. The prior mock-testing framework and the legacy `lib/e2e` and `lib/e2e-proc`
harnesses were removed. Unit tests use the SDK
`TestVM`; E2E tests deploy to an SDK-managed Nitro devnet.

The library keeps its existing component-oriented API and storage layout.
Example contracts now make owner and role authorization explicit. See
[`SECURITY-REVIEW.md`](SECURITY-REVIEW.md) for invariants, findings, PoCs, fixes,
and audit follow-ups.

## SDK changes adopted

The migration was checked against the [Stylus SDK 0.10.10 changelog](https://github.com/OffchainLabs/stylus-sdk-rs/blob/v0.10.10/CHANGELOG.md).
The changes used here are:

- **VM-scoped host access.** Global context/log accessors were replaced with
  `self.vm()` or the relevant call context. The SDK handles storage cache
  coherence around high-level external calls; raw calls choose their cache
  policy explicitly.
- **Stable Rust and aligned dependencies.** SDK 0.10.10, `alloy` 1.7.3,
  `alloy-primitives`/Solidity ABI 1.5.7, and `ruint` 1.17.1+ are pinned or
  aligned. Rust 1.91.0 is the toolchain baseline.
- **SDK testing.** Unit tests use `stylus-test`'s `TestVM` and explicit VM
  setup. The old mock-testing model and `lib/e2e` framework are gone.
- **SDK devnet.** E2E tests use Tokio/Alloy and `cargo stylus deploy` against
  the SDK devnet helper. Each example gets a fresh Nitro node so activation
  pricing and deployment state do not leak across packages.
- **Current public macro behavior.** Public entrypoints use SDK `#[public]`
  blocks and composed traits. The proc macro rejects selector collisions
  within an interface. ABI export checks run for all examples and the
  standalone template.
- **Current byte and ABI types.** SDK `Bytes` uses Alloy bytes; call sites use
  `Bytes::from`/`as_ref`. Decode paths use validating ABI decoders, so callers
  must provide canonical ABI data without trailing bytes.
- **SDK constructor semantics.** The examples pass the final owner explicitly
  because constructor calls run through the configured deployer contract.
  ERC-721 consecutive minting uses an explicit one-time constructor phase;
  code size is no longer treated as a constructor detector.
- **Tooling and artifacts.** Cargo Stylus is workspace-aware, reproducible
  optimizer settings are pinned, fragmented deployments are supported, and
  activation-paused errors are surfaced by the SDK.

The SDK's `contract-client-gen` mode is not enabled on this library's
`#[interface_id]` component traits or its examples. That mode rewrites
`#[public]` trait implementations into host-call stubs; the library traits
carry ERC-165 IDs and contract error types and have a different shape. Enabling
the mode for the library caused signature and associated-type errors. The
unsupported generated feature declarations were removed. A downstream
contract can still use SDK client generation for a separate plain SDK
`#[public]` interface. Never deploy a client-generated artifact. This boundary
is also documented in the upgrade guide.

## Design decisions

- Extensions remain storage components with internal helpers and explicitly
  composed public interfaces. Token balance changes use shared update paths,
  following the composition benefits of Solidity `_update` hooks and Cairo
  storage components while keeping Rust ownership and storage rules visible.
- Authorization remains part of the consuming contract. Examples now protect
  mint, pause, role-admin, upgrade, burn, URI/metadata, and fee-setting methods.
  Core components do not invent a single ownership policy for every consumer.
- UUPS version writes remain a component operation; public wrappers must apply
  the application's upgrade authority and perform the version write during an
  authorized initializer.
- Existing storage fields and order were retained. The ERC-721 enumeration
  fix uses an unused key in the existing nested mapping rather than adding a
  component field. Example-only owner-controlled storage was appended and is
  listed below.
- The ERC-721 wrapper rejects no-code underlying addresses, since an empty
  account must not be treated as a successful token contract.
- The `interface_id` macro rejects duplicate signatures and actual four-byte
  collisions. It cannot compare a custom selector on a trait with a selector
  on a separately expanded `#[public]` implementation; callers must repeat
  the selector and inspect exported ABI.
- Extension documentation compares the relevant Solidity, Cairo, Stellar, and
  Sui Move patterns. It adopts explicit composition/capability checks where
  they improve Rust use without hiding policy in an implicit inheritance-like
  layer.

## Upgrade steps from v0.3.0

Read [`docs/modules/ROOT/pages/upgrade-from-v0.3.0.adoc`](docs/modules/ROOT/pages/upgrade-from-v0.3.0.adoc)
for the full consumer guide. The primary source changes are:

| v0.3.0 | SDK 0.10.10 migration action
|---|---|
| Old global environment accessors and mock-framework tests | Use VM accessors and `stylus-test` `TestVM`; adapt test setup to explicit sender, contract address, and storage. |
| `lib/e2e`/`lib/e2e-proc` E2E macros | Use the SDK devnet and Tokio/Alloy test code; run packages through `scripts/e2e-tests.sh`. |
| SDK/Alloy versions older than 0.10.10/1.7.3 | Align toolchain and direct Alloy versions to the workspace pins. |
| Tuple-style `Bytes` and permissive decode assumptions | Use Alloy `Bytes` constructors/accessors and canonical ABI input; malformed or trailing bytes now fail validation. |
| `IErc4626` read-only methods taking `&mut self` | Implement read-only methods with `&self`. |
| Custom `#[selector]` on the interface trait only | Repeat the same override in the public implementation and check ABI; the SDK does not compare separate macro expansions. |
| `Erc721Enumerable` index sentinel could collide with token ID zero | New state enumerates ID zero. On existing proxies, call `_initialize_owner_enumeration(owner, erc721)` once for each owner whose legacy holdings include ID zero. No surrounding storage slot was added. |
| ERC-721 consecutive detection based on code size | Call `_mint_consecutive_batches` from the constructor; runtime calls now fail. Existing proxies start with the new namespaced phase closed. |
| Implicit owner from deployer caller | Pass explicit final `initial_owner` to the example constructors. For existing proxies, call `initializeOwner(address)` atomically with the implementation upgrade. |
| Example storage without owner or one-time initialization guard | New `Ownable` and `StorageBool` fields are appended for the examples listed in the upgrade guide; no prior field is reordered. Compare generated storage layouts before deploying an upgrade. |
| UUPS example wrapper exposed `set_version()` without authority | Require the proxy owner and initialize version atomically with deployment/upgrade. |
| ERC1155 supply burns and example NFT burns had no caller authorization | Burns require owner/operator approval; example `burn(uint256)` remains in the ABI but now enforces it. |
| ERC721 wrapper could use a no-code underlying address | Deposit, withdrawal, and receiver-hook entrypoints reject no-code underlying addresses with `ERC721UnsupportedToken`. |
| ERC2981 zero denominator/overflow behavior | Zero denominator now means the documented 10,000 basis-point default; royalty math uses a full-precision intermediate and rounds down. |
| Empty/malformed `ecrecover` result | ECDSA returns an error for malformed precompile output instead of treating it as the zero address. |

The upgrade guide records constructor argument order, example ABI changes,
owner initialization, the ID-zero backfill, selector checks, and all security
behavior changes. It also calls out first-caller-wins initialization: do not
leave an uninitialized proxy callable between upgrade and initialization.

## Gas measurements

Measurements were run on the devnet before and after migration using the same
benchmark scenarios. Values are gas units; lower is better. The bench harness
reports cached and uncached transactions; where they differ, the migrated
column records their arithmetic mean, matching the baseline report.

| Benchmark | v0.3.0 | SDK 0.10.10 | Change
|---|---:|---:|---:|
| AccessControl `hasRole` | 37,149 | 30,974 | -16.6% |
| ERC-20 `transfer` | 68,038 | 63,003 | -7.4% |
| ERC-20 `transferFrom` | 53,194 | 47,968 | -9.8% |
| ERC-721 `transferFrom` | 88,189 | 92,651 | +5.1% |
| ERC-721 mint | 164,211 | 165,742 | +0.9% |
| ERC-1155 `mintBatch` | 121,713 | 117,139 | -3.8% |
| Merkle proof verification | 32,311 | 28,817 | -10.8% |
| Poseidon hash | 31,846 | 25,816 | -18.9% |

ERC-721 transfer/mint rose slightly after the correctness and reentrancy
guards. No gas optimization was accepted at the expense of those invariants.

## WASM size and activation

The optimizer is pinned to Binaryen/`wasm-opt` 116 with
`--enable-bulk-memory -O3`. The migrated set includes 33 example contract
artifacts; the template is measured separately.

| Artifacts | v0.3.0 | SDK 0.10.10 |
|---|---:|---:|
| Raw example artifacts | 32, total 2,299,900 bytes (21,457–125,487) | 33, total 2,473,532 bytes (24,355–128,181) |
| Optimized/deployable example artifacts | 31 optimized, total 1,142,442 bytes (9,095–69,453) | 33 deployable, total 1,905,517 bytes (17,574–99,919) |
| Standalone crypto ERC-20 template | Not in the v0.3.0 example set | Raw 112,652; Binaryen output 87,029; SDK check reports 34,036 bytes / 2 fragments |
| SDK activation size from final check | Not measured with the same SDK check | 33 examples: 7,330–34,599 bytes; template: 34,036 bytes; all 34 activated |

The v0.3.0 and migrated artifact counts and optimizer recipes differ, so the
totals are not a direct like-for-like percentage comparison. Every migrated
example and the standalone template passed SDK devnet activation; the largest
deployable example is below the configured size limit.

## Evidence and checks

### Environment

Preflight found all required project tools at the requested versions. An
optional Rust 1.96.0 probe during fuzz setup caused rustup to auto-install its
missing components; I immediately uninstalled that toolchain. The fuzz smoke
runs used the existing Rust 1.91.0 with `RUSTC_BOOTSTRAP=1`; no extra toolchain
remains installed.

| Tool | Version observed
|---|---|
| Rust / Cargo | stable 1.91.0; rustfmt 1.8.0 |
| Cargo Stylus / SDK | 0.10.10 |
| Docker daemon | 29.2.1, running |
| `wasm-opt` | 116 |
| `wasm-tools` | 1.262.0 |
| `cargo-fuzz` | 0.13.2 |
| `cargo-llvm-cov` | 0.9.1 |
| `cargo-audit` | 0.22.2 |
| `cargo-deny` | 0.20.2 |
| `pdftotext` (audit review) | 26.10.0 |

### Required checks

| Check | Final result
|---|---|
| `cargo +1.91.0 fmt --all -- --check` and template fmt | Pass |
| `cargo +1.91.0 check --locked --workspace --all-targets` | Pass |
| `cargo +1.91.0 clippy --locked --workspace --all-targets -- -D warnings` | Pass |
| `cargo +1.91.0 test --locked --workspace --all-targets` | Pass: 126 targets, 738 passed, 0 failed, 0 ignored |
| `cargo +1.91.0 test --locked --workspace --doc` | Pass |
| `cargo +1.91.0 check --locked --workspace --all-features` | Pass for non-test targets; emits 66 missing-doc warnings from SDK-generated `#[public]` items |
| Template check/test/clippy, format, and ABI export | Pass; 5 template unit tests passed; all-features check has the same generated-item warnings |
| `./scripts/check-abi.sh` | Pass: 35 ABI projects |
| `env -u RPC_URL ./scripts/e2e-tests.sh` | Pass: 30 packages, 49 test binaries, 521 passed, 0 failed, 0 ignored |
| `env -u RPC_URL ./scripts/bench.sh` | Pass on a fresh SDK devnet; all 11 benchmark groups completed |
| `env -u RPC_URL ./scripts/devnet.sh run -- ./scripts/check-wasm.sh` | Pass: 33 examples plus template; all 34 activated |
| Fuzz smoke (Keccak, Merkle, EdDSA) | Pass with `--sanitizer none`, no crash: 4,424,091 / 735,579 / 605,998 inputs in 6-second runs |
| `cargo audit` | Five transitive devnet/test-tool advisories remain; see below |

The source scan found no legacy mock-framework references in code, global accessors, or
commented-out assertions. The normalized E2E scenario inventory contains all
490 v0.3.0 scenarios. The migrated suite has 521 cases; the old
`set_version_doesnt_revert_if_called_more_than_once` case is represented by
`set_version_remains_idempotent` plus explicit initializer-replay rejection.

Coverage on the native workspace run increased from the v0.3.0 baseline of
82.61% lines / 83.60% regions / 74.85% functions to 88.16% lines / 89.95%
regions / 79.54% functions. The 100% target was not reached. Native coverage
does not instrument Wasm executed in the devnet, and the devnet helper itself
has no unit coverage; this is recorded rather than extrapolated as measured
coverage.

The separate fuzz lockfile was used for short smoke runs (6 seconds per
target). The installed nightly `cargo-fuzz` toolchain could not compile its
`branches` dependency because it calls unavailable `core::intrinsics::abort_immediate`;
the smoke runs used the already-installed stable toolchain with
`RUSTC_BOOTSTRAP=1`. The Apple AddressSanitizer runtime hung in its static spin
mutex during a default-sanitizer smoke run, so that run was interrupted and the
successful smoke runs used `--sanitizer none`. The optional probe toolchain was
uninstalled immediately.

`cargo audit` reports remaining advisories in `stylus-tools` 0.10.10's
devnet/test dependency graph: `brotli-sys` (RUSTSEC-2021-0131),
`hickory-proto` (RUSTSEC-2026-0119 and RUSTSEC-2026-0118), `rkyv`
(RUSTSEC-2026-0235), and `tokio-tar` (RUSTSEC-2025-0111). The `bytes`
RUSTSEC-2026-0007 advisory was fixed by lockfile updates to 1.11.1. The
standalone template lockfile passed audit. These devnet/test dependencies are
not in deployed contract WASM. `cargo deny` did not produce a policy result
because no repository `deny.toml` is present; its default license policy
rejects licenses without configured allowances.

### Additional matrix check

`cargo check --workspace --all-features --all-targets` is not a supported
combination: enabling `export-abi` while compiling library unit-test fixtures
causes the SDK macro to emit duplicate `print_from_args` functions and ABI
structs for multiple test-only contracts in one module. The normal all-target
test and strict clippy commands pass, ABI export is checked per contract, and
the all-features non-test workspace check passes. This limitation is explicit
so the failed exploratory command is not mistaken for a passing check.
