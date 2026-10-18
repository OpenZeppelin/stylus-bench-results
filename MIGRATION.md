# Stylus SDK 0.10.10 migration

This report records the migration from v0.3.0 and the validation performed on
stable Rust 1.91.0. The detailed consumer steps and storage warnings are in
[`docs/modules/ROOT/pages/upgrading.adoc`](docs/modules/ROOT/pages/upgrading.adoc).

## Environment

Preflight passed before repository work began. The environment reported:

| Tool | Version |
|---|---:|
| `rustc` / `cargo` | 1.91.0 |
| `rustfmt` | 1.8.0-stable |
| `git` | 2.54.0 |
| `cargo-stylus` | 0.10.10 |
| `stylus-sdk` / `stylus-tools` | 0.10.10 |
| `cargo-fuzz` | 0.13.2 |
| `cargo-llvm-cov` | 0.9.1 |
| `ripgrep` | 15.2.0 |
| Docker client / server | 29.2.1 / 29.2.1, daemon running |
| Nitro devnet image | `offchainlabs/nitro-node:v3.11.1-8512b8c` |
| `wasm-opt` | 116 |
| `wasm-tools` | 1.262.0 |
| `pdftotext` (audit report review) | 26.10.0 |
| Fuzz toolchain | Rust/Cargo nightly 1.92.0 (2025-09-22) |
| Rust target | `wasm32-unknown-unknown` installed |
| Python | 3.13.7 |
| Node.js / npm | 24.6.0 / 11.5.1 |
| `@openzeppelin/docs-utils` | 0.1.5 |

No tools were installed during the work.

## Migration choices

### Stylus SDK changes adopted

The workspace and template pin Stylus SDK, `stylus-tools`, and `cargo-stylus`
to 0.10.10, with the SDK's Rust 1.91.0 toolchain file. Alloy-facing public
types moved to Alloy 1.5 primitives and Alloy 1.7 providers. `ruint` is held at
1.17.1 or newer.

| SDK change | Adoption |
|---|---|
| Explicit host access (`HostAccess` and `self.vm()`) | Replaced contract-global message, block, transaction, address, balance, code-size, and log accessors throughout production contracts and examples. Host-dependent traits and proxy/storage helpers now receive the contract context. |
| Explicit `Call` contexts | Typed external interfaces pass the VM and `Call::new()` for views or `Call::new_mutating(self)` for writes. High-level SDK calls provide the SDK's storage-cache flush behavior. |
| SDK `TestVM` testing | Replaced the legacy fixtures with SDK TestVM storage, host, log, and call controls. Added a narrow `vm_test` fixture macro for common local setup. A regression test exposed that SDK 0.10.10's mock response was shared across registered calls; a pinned, test-only `stylus-test` patch publishes the response for the call that matched. Production code is not patched. |
| SDK devnet tooling | Replaced `lib/e2e` and `lib/e2e-proc` with `stylus-tools` TestContext, Testcontainers Nitro nodes, and a serial package runner. Benchmarks deploy to fresh SDK-managed devnets. |
| SDK ABI and selector checks | Updated public and inherited routing traits with `#[public]`, used validating ABI decode paths, and added compile-time four-byte selector-collision checks to `#[interface_id]`. The upgrade guide calls out the remaining `#[interface_id]` selector-vs-router limitation. SDK 0.10.3 also fixes dynamic tuple return decoding; no library ABI needed that shape. |
| Cargo-aware Stylus projects | Added `Stylus.toml` files to every contract example and the crypto ERC-20 template, and migrated activation/deployment to the SDK tooling. |
| Reproducible deployment and fragmentation | SDK 0.10.6 adds a Docker build path for deploy/verify. Devnet fixtures invoke `cargo stylus deploy --no-verify` for fast local test builds; the upgrade guide directs release deployments through the normal reproducible verification path. SDK 0.10.1 fragmented deployment is used automatically for compressed artifacts larger than one code chunk; every example was checked and the Nitro devnet is upgraded to ArbOS 60 before activation tests. |
| Reproducible optimizer recipes | SDK 0.10.9 can hash and replay pinned Binaryen settings for deploy and verification. The repository leaves optimization unset because Binaryen 116 output contains a `DataCountSection` rejected by the SDK devnet Nitro image used here. Deployable unoptimized Rust Wasm is used; gas is measured on that same artifact path. |
| Reentrancy and cache safety | SDK 0.10.5 deprecated its `deny_reentrant` guard because high-level calls flush storage caches; SDK 0.10.10 still generates that guard unless the `reentrant` feature is set. The flash mint example opts in because its standard receiver callback reenters the contract; other examples leave the guard enabled. Stateful library calls use the SDK cache-flushing call path. |
| `contract-client-gen` | Not enabled on storage component crates. That feature rewrites `#[storage]` and `#[public]` into address-based client stubs, which is useful for client-only code but incompatible with a crate that must also compile as a contract. Example manifests declare the empty feature name only so Cargo recognizes the SDK's conditional compilation. |
| SDK 0.10.10 activation status | The devnet runner reports the SDK's paused-activation state distinctly; runners should retry after activation resumes. |
| Features not needed by this component library | The SDK supports tuple/nested returns, custom storage slots, storage integer helpers, codehash keepalive, and client generation. Existing contracts do not need new tuple ABI shapes or keepalive calls; ERC-1967 already owns custom slots; storage helpers remain explicit component APIs. `contract-client-gen` is not enabled because it rewrites `#[storage]` and `#[public]` component crates into address-based client stubs. |

The upstream release references are the [SDK 0.10.10 changelog](https://github.com/OffchainLabs/stylus-sdk-rs/blob/v0.10.10/CHANGELOG.md), its [testing documentation](https://docs.rs/stylus-sdk/0.10.10/stylus_sdk/testing/index.html), and the [stylus-tools documentation](https://docs.rs/stylus-tools/0.10.10/stylus_tools/).

### Extension composition

ERC20, ERC721, ERC1155, Pausable, Ownable, enumerable, supply, URI, and vault
features remain explicit storage components with explicit trait forwarding.
Consumers choose and compose the components in their own `#[storage]` struct;
the library does not hide storage or insert implicit transfer hooks. This keeps
the same composability advantages found in Solidity's opt-in extensions, Cairo's
component model, Stellar/Soroban's explicit contract interfaces, and Sui Move's
module composition while making Stylus storage order visible to Rust users.

The comparison sources were the official [Solidity ERC20 API](https://docs.openzeppelin.com/contracts/4.x/api/token/erc20),
[Cairo ERC20 components](https://docs.openzeppelin.com/contracts-cairo/3.x/api/erc20),
[Stellar contracts](https://docs.openzeppelin.com/stellar-contracts), and
[Sui Move contracts](https://docs.openzeppelin.com/contracts-sui). The main
design choice was to preserve explicit Rust component wiring and improve its
host/call ergonomics, rather than add inheritance-like implicit hooks that
would obscure storage layout and callback ordering.

## Behavior and storage changes

The standard ERC20, ERC721, ERC1155, proxy, and event selectors remain stable.
The upgrade guide contains the full old-to-new host API table and consumer
actions. The notable behavior changes are:

* ERC721 enumerable derives owner-index bounds from the core ERC721 balance.
  This makes token ID zero enumerable without a new storage field or extra
  writes on transfers. Proxy storage layout and existing enumeration data stay
  unchanged. Rust consumers forward `token_of_owner_by_index` from the composed
  contract and pass its `Erc721` component; the Solidity ABI is unchanged.
* BeaconProxy keeps its legacy ordinary storage field reserved and reads the
  canonical ERC-1967 beacon slot. Existing v0.3.0 constructors populated that
  slot, so the proxy's storage layout and ABI remain stable.
* Consecutive ERC721 mint now rejects calls after construction, while allowing
  the SDK's canonical `StylusDeployer` constructor context.
* ERC-4626 rejects decimal offsets whose share decimals or virtual share amount
  overflow. The Rust constructor helper now returns `Result`; the ABI
  constructor arguments are unchanged. Existing proxy state is not rewritten,
  so inspect stored offsets before upgrade.
* ERC-2981 uses the standard 10,000 denominator when fresh storage reads zero
  and uses full-precision royalty arithmetic. Vesting schedule arithmetic also
  uses full-precision multiplication/division.
* `BabyJubjubConfig::GENERATOR` now returns EIP-2494's `Base8` point, the
  generator of the scalar field's prime-order subgroup. The old point `G`
  generated the full curve of order `8l`; code that depended on that mismatched
  point must update to the subgroup generator or implement full-curve
  arithmetic explicitly. This changes the Rust crypto API behavior, not any
  contract ABI or storage.
* Safe ERC-20 accepts empty return data or a canonical true word; false and
  malformed words fail. Trailing bytes after the first word remain ignored.
* ERC721Wrapper rejects no-code underlying addresses. ERC20Wrapper and ERC-4626
  retain nominal accounting and require exact-transfer, non-rebasing assets;
  fee-on-transfer assets are documented as unsupported.
* The SafeERC20 example takes an explicit initial owner, uses the library's
  `Ownable` component, and restricts every helper. It appends that component
  after its existing SafeERC20 storage. The example owner is fixed because the
  fixture does not expose ownership transfer; consumers should select their
  governance address and expose the ownership policy they need. Existing
  proxies of this example need a deployment or initializer migration before
  enabling token-moving methods.

The complete breaking-change inventory and upgrade instructions are in the
upgrade guide. No production storage field was added or reordered in the SDK
migration; the SafeERC20 example's appended owner field is described above.

## E2E scenario inventory

The v0.3.0 tag has 490 E2E scenario functions. The migrated suite retains every
success and failure scenario; ten ERC-4626 operation tests were renamed to
reflect valid-offset arithmetic tests, and one vesting overflow assertion now
uses the full-precision expected result. Five additional devnet scenarios
cover smoke deployment, invalid beacon, beacon initializer failure, invalid
vault offset, and Merkle proof verification. The SafeERC20 example authorization
PoC adds one more scenario, for 496 unique example E2E scenarios. All 496 passed
across the package runner and final targeted reruns; the template activation
smoke test passed separately (1/1). The final ERC721 enumerable implementation
also passed its full 64-test E2E package rerun, and SafeERC20 passed 85/85 after
the authorization test was added. The first flash-mint migration run exposed
seven callback failures; the SDK `reentrant` feature fix is recorded as C-12,
and the package then passed 20/20.

## Gas and Wasm size

Measurements use the v0.3.0 tag and the migrated workspace, both on stable
Rust 1.91.0 and the repository's available Nitro devnet. The baseline
benchmark's AccessControl sequence attempted to revoke a role before granting
it; that benchmark setup is corrected only in the temporary v0.3.0 worktree so
the same successful operation sequence can be measured. The repository's
baseline source is not modified by that correction.

The 49 matched contract operations were measured in cached and uncached modes
using the same SDK-managed Nitro image and benchmark harness. The two Solidity
reference rows have no cached measurement and are excluded from the comparison.
Across the matched paths, the mean per-operation change is **-0.60%** in both
modes (median -0.43%; range -2.53% to +0.51%). The sum of the reported gas
values is 2,966,886 to 2,949,703 cached and 2,966,946 to 2,949,691 uncached.
The sums are a compact regression check, not an estimate of a single
transaction that executes every operation.

| Operation | v0.3.0 cached | 0.10.10 cached | Change |
|---|---:|---:|---:|
| ERC20 `transfer` | 68,038 | 67,767 | -0.40% |
| ERC20 `transferFrom` | 53,194 | 52,763 | -0.81% |
| ERC721 `safeTransferFrom` | 117,267 | 116,964 | -0.26% |
| ERC721 `transferFrom` | 88,189 | 87,337 | -0.97% |
| ERC721 `mint` | 164,211 | 163,599 | -0.37% |
| ERC721 `burn` | 62,053 | 61,363 | -1.11% |
| ERC1155 `safeTransferFrom` | 73,896 | 73,677 | -0.30% |
| ERC1155 `safeBatchTransferFrom` | 131,894 | 130,411 | -1.12% |

The ERC721 token-zero repair was first implemented with an appended owner-count
mapping. That raised `safeTransferFrom` gas 23.6% and `transferFrom` 11.4% in an
intermediate benchmark. The final design bounds owner indexes with the core
ERC721 balance, preserving the old layout and eliminating the extra transfer
writes; its final measurements are in the table above. Baseline AccessControl
benchmark setup was corrected in the detached v0.3.0 worktree to grant the
tested role before measuring `revokeRole`; without that setup the old benchmark
attempted an invalid revoke. The v0.3.0 source checkout was not changed by the
correction.

The benchmark source selector and prebuilt-Wasm deployer let both versions run
through the same SDK devnet. The v0.3.0 benchmark uses the old compiled Wasm;
small temporary setup helpers in the detached baseline checkout reproduce
constructor state because the SDK raw-Wasm deploy path does not execute the old
constructor arguments. Those helpers are outside measured functions and do not
change the baseline storage layout. Baseline source was restored after the
measurements. Binaryen 116 optimization is unset: its output contains a
`DataCountSection` rejected by this Nitro build, so deployable and measured
artifacts use unoptimized Rust Wasm.

All 32 example Wasm artifacts were compared against v0.3.0 using the same
stable Rust 1.91.0 target and unoptimized release profile. Maximum raw size
increased from **125,487 bytes** (ERC-4626) to **128,086 bytes** (ERC-4626), a
2.1% increase. The added ERC-20 ownership gates make SafeERC20 the largest
relative increase (58,806 to 79,514 bytes); it remains below the activation
limit. The crypto-ERC20 template is 112,142 raw bytes and has no v0.3.0
counterpart.

`scripts/check-wasm.sh` passed all **33** configurations (32 examples plus the
template) against an SDK-managed local Nitro devnet. The largest compressed
on-chain code was **34,861 bytes** and used two fragments; 15 artifacts used two
fragments, all within SDK/Nitro limits. All 32 example configurations were
activated in the 30-package E2E replay; proxy tests also deploy the invalid
ERC-1967 and new-version UUPS support examples. The template activation smoke
test passed separately. Binaryen 116 optimization is unset: its output
contains a `DataCountSection` rejected by this Nitro build, so deployable and
measured artifacts use unoptimized Rust Wasm.

## Validation evidence

The following commands and runs completed on the final migration tree unless
otherwise noted:

| Check | Result |
|---|---|
| `cargo llvm-cov --workspace --all-targets --locked` (also executes the all-target test suite) | Passed: 884 tests across 118 test binaries; zero failures. |
| `cargo test --jobs 1 --workspace --doc --locked` | Passed; rustdoc examples compile and run. |
| `cargo clippy --jobs 1 --workspace --all-targets --locked -- -D warnings` | Passed. |
| Strict Clippy with `--features e2e` for all 30 E2E packages | Passed. |
| `cargo fmt --all -- --check`; fuzz and template formatting checks; `git diff --check` | Passed. |
| `cargo doc --workspace --no-deps --locked` | Passed. |
| `cd docs && npm run docs` | Passed; site generated in `docs/build/site`. |
| Legacy API scan for Motsu/global accessors/commented E2E tests | No hits in library source or tests. The old accessor names occur only in the v0.3-to-new API mapping in the upgrade guide. |
| v0.3.0 crypto differential vectors | Passed against the checked-in baseline vectors. |
| `cargo +nightly fuzz run --sanitizer none merkle -- -max_total_time=30 -max_len=4096` | Passed: 4,265,900 executions; no crash. |
| `cargo +nightly fuzz run --sanitizer none keccak -- -max_total_time=30` | Passed: 20,926,798 executions; no crash. |
| SDK devnet E2E replay | All 496 unique example scenarios passed, including every retained v0.3.0 success/failure scenario; final SafeERC20 package 85/85 and final ERC721 package 64/64. The crypto ERC-20 template activation smoke test passed 1/1. |
| `scripts/bench.sh` and matched v0.3.0/current benchmark replay | Passed: 49 contract operations compared in cached and uncached modes. Mean measured gas change was -0.60%. |
| `scripts/check-wasm.sh` against SDK-managed local Nitro | Passed all 33 configurations: 32 examples and the crypto ERC-20 template; all activated and fit within SDK/Nitro code limits. |
| Coverage (`cargo llvm-cov clean --workspace` followed by `cargo llvm-cov --workspace --all-targets --locked`) | Passed and exceeded v0.3.0: lines 98.18% vs 82.61%; regions 98.18% vs 83.60%; functions 96.71% vs 74.85%. The 100% target was not reached. Remaining gaps are concentrated in crypto const/inline arithmetic that LLVM does not attribute despite runtime assertions; Poseidon2 and projective branches that constructors or available field types make unreachable; macro-generated declaration lines; and impossible-state error conversions in ERC4626, ERC721, and vesting. `EnumerableSet`'s U256 fallback cannot be reached because `StorageVec::len()` is `usize`. |

The default macOS AddressSanitizer runtime stalled during initialization
before entering the Merkle target. The fuzz runs above therefore used
libFuzzer without ASan; they exercised the targets but do not provide sanitizer
memory-error coverage. The first migrated flash-mint E2E run failed seven
callback scenarios; enabling the SDK `reentrant` feature fixed them, and all
20 package scenarios then passed. The initial public devnet activation check
reported activation paused, so the final 33-configuration activation check
used the SDK-managed local Nitro node. No failed result is omitted from this
record.
