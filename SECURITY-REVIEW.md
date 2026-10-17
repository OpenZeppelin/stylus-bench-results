# Security review

## Scope and method

Reviewed the v0.3.0-to-0.4.0 changes and the surrounding code needed for the SDK 0.10.10 migration: contract storage and authorization, token accounting and callbacks, wrappers and vaults, EIP-1967/UUPS/beacon flows, crypto input validation and arithmetic, proc-macro selector generation, devnet test support, examples, and all three audit reports under `audits/`.

The review combined source-level invariant checks, v0.3.0 finding-by-finding verification, regression PoCs written and run before their fixes, SDK devnet E2E scenarios, native/property tests, 15-second libFuzzer runs, a behavioral comparison against the v0.3.0 ERC20 WASM, and dependency auditing. No mutation testing or formal verification was used. New state/error PoCs are in separate commits before their corresponding fixes, as listed below.

## Invariants reviewed

| Area | Invariants exercised |
| --- | --- |
| ERC-20 core | Total supply changes with mint/burn; balances move by the requested amount; zero sender/receiver/spender/approver checks hold; allowance is consumed only for a successful delegated transfer/burn; events agree with state. |
| ERC-20 extensions | Cap bounds supply growth; pause blocks configured operations; permit uses the correct EIP-712 domain and nonce; flash mint repays and respects the composed cap; wrapper liabilities are backed by the underlying token; SafeERC20 accepts only empty or canonical `true` return data. |
| ERC-4626 | Share/asset conversions preserve rounding direction and bounds; deposit/mint/withdraw/redeem update shares and assets consistently; max views reflect limits; external asset failures do not become silent success. |
| ERC-721 and extensions | Each token has one owner; owner balances and approvals agree; invalid transfers/burns do not change state if their error is caught; receiver callbacks use the canonical selector; global and owner enumeration include token ID zero; consecutive mint behavior remains constructor-only by contract. |
| ERC-1155 and extensions | Per-ID balances, authorization, supply counters, and transfer/burn events agree; batch lengths and balances are validated; repeated IDs aggregate correctly; receiver acceptance is checked before a successful public operation. |
| Access control / ownership | Only the configured role admin changes roles; enumerable members match held roles; owner and pending-owner transitions are authorized and cannot enter an unrecoverable state accidentally. |
| Proxies | EIP-1967 implementation/admin/beacon slots do not overlap ordinary storage; targets have code and beacons return a valid implementation; failed setup does not leave the proxy on the new target; UUPS UUID/context/authorization checks hold; initialization is one-time, including legacy proxies and renounced ownership. |
| Vesting | Released amounts are monotonic and cannot exceed vested or total allocation; timestamps and duration bounds are handled. |
| Merkle / collections / checkpoints | Invalid proof shapes fail; commutative hashing assumptions are documented; set membership/indexing stay consistent; range reads are bounded; checkpoint ordering and lookup boundaries are correct. |
| Crypto | Field/scalar inputs are canonical; curve points are on curve and in the intended subgroup; identity keys are rejected for EdDSA verification; signature scalars are canonical; invalid Poseidon2 parameters cannot define a colliding non-permutation; integer conversions and zero/overflow cases are checked. |

## New findings fixed in this migration

Severity is my assessment of the affected contract/library behavior. Each regression test was committed before the fix. Where a test-only reproducer does not model a public router, the issue is specifically a composition hazard: returning Rust `Err` does not itself revert the EVM frame when the caller catches it.

| Severity | Finding and trigger | Impact and fix | PoC → fix |
| --- | --- | --- | --- |
| High | **Capped flash mint could exceed the cap transiently.** Compose `Capped` with the generic flash-mint path and request more than `cap - total_supply`. | The callback sees supply above the configured maximum and can use those tokens in downstream protocols. The capped helpers now bound `max_flash_loan` and enforce the cap before loan minting. | `b6817ba` → `5e3e34f`; E2E `flash_loan_must_not_exceed_the_composed_supply_cap`. |
| High | **UUPS initializer replay could replace privileged state.** Call an initializer again through a proxy after it was initialized. | An untrusted caller could overwrite owner/configuration and take control. The initializer now sets and checks a namespaced one-time marker. | `184f7a5` → `51447b7`. |
| High | **A legacy proxy with a renounced owner could be mistaken for an uninitialized proxy.** Upgrade a v0.3 proxy whose owner is zero, then call the new implementation's initializer. | An attacker could claim the new implementation's privileged state. The UUPS example recognizes legacy version state independently of the owner, and `setVersion` is owner-only. | `476ca23` → `cbc3281`; legacy-state unit test. |
| Medium | **BeaconProxy ordinary storage collided with the beacon address.** Deploy an implementation with meaningful data at slot zero; proxy setup could overwrite it, and later fallback read the wrong beacon source. | Corrupted application state or broken proxy routing. BeaconProxy now reads the EIP-1967 beacon slot and retains the old ordinary field as reserved. Already-overwritten slot-zero data cannot be recovered automatically. | `91c818e` → `0bd2255`; `implementation_scalar_must_not_overwrite_beacon_address`. |
| Medium | **ERC-721 enumerable tracking missed token ID zero.** Mint token zero and enumerate its owner/global token list. | Enumeration could omit a token even though ownership and balance showed it existed. A namespaced presence marker distinguishes a valid zero index from an absent mapping entry. Existing deployed ID-zero records need a backfill. | `f70228a` → `d19c594`. |
| High | **EdDSA accepted the identity verifying key.** Verify with the identity point as the public key and a trivial signature. | An attacker could forge a signature for that key. Verification-key construction now rejects identity, malformed, and non-prime-order points. | `8fa1892` → `991a6b3`; crypto unit tests. |
| Medium | **EdDSA accepted noncanonical field/scalar encodings.** Supply field coordinates outside the base-field modulus or a signature scalar at/above the subgroup modulus. | Multiple encodings could alias a value or enter invalid arithmetic paths. Inputs are rejected unless canonical. | `48545c3` → `4773618`; crypto unit tests. |
| Medium | **ERC-721 wrapper could mint or burn without a contract backing the asset.** Configure an EOA as the underlying token or call its receiver callback directly. | A caller could create unbacked wrapper tokens or burn wrappers without transferring an underlying NFT. The wrapper checks that the underlying address has code on deposit, withdrawal, and callback paths. | `9d231b4` / `128a91d` → `5950766`; wrapper E2E tests. |
| Low | **ERC-20 wrapper recovery error named the wrong token.** Make the underlying `balanceOf` call fail during recovery. | The error encoded the wrapper address, misleading a caller or monitoring system. It now reports the configured underlying address. | `8fb3451` → `a2a6afe`. |
| Medium | **Poseidon2 accepted a non-permutation S-box exponent.** Construct custom parameters with `gcd(D, p - 1) != 1`. | The exponent is not invertible, so distinct inputs can collide in the S-box and invalidate hash assumptions. Construction now rejects it. | `f164df1` → `d88ea2`; `invalid_sbox_makes_the_sponge_hash_collide`. |
| Medium | **Poseidon2 accepted invalid full-round parameters.** Supply an odd full-round count or fewer than the configured security minimum. | The implementation could be mistaken for a secure permutation while omitting required rounds. Constructor checks reject these parameter sets. | `4ccbec4` → `08b08c2` / `04805e1`; parameter regression tests. |
| Medium | **ERC-20 allowance could be spent before a failing delegated transfer.** A composing contract catches a transfer error after `_spend_allowance` has already written. | The public caller could return success with an unexpectedly reduced allowance. Allowance spending now happens only after the transfer succeeds. | `4d2ce2d` → `88ff102`; caught-error regression test. |
| Medium | **ERC-721 returned errors could leave partial state/logs when caught.** Trigger duplicate mint, a transfer whose supplied `from` does not own the token, or burn of a nonexistent token; catch the returned error. | The earlier path could write ownership/balances or emit a misleading `Transfer` before returning `Err`. The operations now validate before mutation/logging. | `b7d554d` / `32e25e7` → `80619a9` / `0569e9d`; tests in `contracts/tests/caught_errors.rs`. |
| Medium | **ERC-1155 batch failure could partially burn earlier IDs when caught.** Use a batch whose later ID has insufficient balance; duplicate IDs also require aggregate validation. | Earlier ID balances/events could change despite a returned error, or repeated IDs could pass independent checks but exceed the aggregate balance. The code validates aggregated amounts before any write. | `b7d554d` → `80619a9`; invalid-batch and duplicate-ID regression tests, including `5747330`. |
| Medium | **ERC-1967 upgrade failure could leave partial proxy state or misleading event order.** Catch invalid target, nonpayable, or initializer/delegate-call errors after a candidate slot write. | A proxy could remain pointed at the failed candidate, or emit an upgrade event before setup succeeds. Helpers validate targets, restore prior slots on setup failure, preserve v0.3 error precedence, and emit upgrade events only after successful setup. | `91368fc` → `e6fc042`; `932b791` → `3f60b83`; targeted proxy unit tests. |
| Low | **Upgradeable beacon constructor could leave owner set after an invalid implementation error was caught.** Call the constructor helper on composed storage and catch its `Err`. | A caller could observe partially initialized beacon state. It now validates owner and implementation before writing. | `b3c76ba` → `dae7494`. |
| Low | **Interface selector metadata could diverge for conditional methods and selector collisions.** Compile traits with `cfg`-conditional functions or colliding selectors. | ERC-165 IDs/routing metadata could disagree with the functions built for a target. The macro now carries conditional attributes and detects selector collisions within the checked trait/public block. Cross-block and custom-selector/implementation matching is still a limitation. | `abc3de2` → `0964fea`; `cacf84d` → `b125b59`; raw-byte selector correction in `7057cca`. |

## Residual security and dependency risks

### Composition and crypto API hazards

* **Caught-error atomicity (Low/Medium, documented API hazard).** Trigger: a consumer catches a library `Err` and later returns `Ok`. Writes/logs already made in the caller's frame and successful external subcalls remain. The library prevalidates the token/proxy failure cases listed above, but it cannot make arbitrary user callbacks, external tokens, or vault operations transactional. Propagate with `?` when atomic behavior is required. A token that mutates and then returns `false` is also outside the caller's ability to roll back if caught.
* **Variable-time EdDSA signing (Low, new review note).** The generic twisted-Edwards scalar multiplication loops over a trimmed secret scalar and branches on each bit (`BitIteratorBE` and `mul_projective`). A local timing observer may learn information about a signing key. The signing API is intended for off-chain use; do not expose it to a hostile timing environment. A constant-time signing implementation is a separate design change.
* **Consecutive minting (Low, API constraint).** `_mint_consecutive` intentionally omits per-token transfer events and callbacks; consumers must call it only during construction, as the docs state.
* **Selector matching (Low, acknowledged audit limitation).** `#[interface_id]` cannot currently prove that a custom selector declared on a trait matches every downstream `#[public]` implementation, and collisions across separate public blocks are not checked.

### Dependency audit

`cargo audit` scanned 723 locked dependencies. It exited nonzero with five vulnerabilities and ten warnings, all in the host-side SDK devnet/test stack (`stylus-tools` → Wasmer/testcontainers); the deployed no-std contract dependency set does not include these host tools.

| Severity | Dependency finding | Current exposure / disposition |
| --- | --- | --- |
| Medium | `brotli-sys 0.3.2`, [RUSTSEC-2021-0131](https://rustsec.org/advisories/RUSTSEC-2021-0131): integer overflow in bundled Brotli C library; no fixed upgrade listed. | Host-side Wasmer/devnet dependency. No upstream-fixed version was available in the selected SDK stack. |
| Medium | `hickory-proto 0.25.2`, [RUSTSEC-2026-0119](https://github.com/hickory-dns/hickory-dns/pull/3613): CPU exhaustion during message encoding. | Host-side DNS resolution. Advisory requests `>=0.26.1`, but the locked `hickory-resolver 0.25.2` dependency constrains the compatible series. |
| Medium | `hickory-proto 0.25.2`, [RUSTSEC-2026-0118](https://github.com/hickory-dns/hickory-dns/pull/3597): NSEC3 cross-zone response can cause an unbounded validation loop; no fixed upgrade listed. | Same host-side DNS path; not contract WASM. |
| High | `rkyv 0.7.46`, [RUSTSEC-2026-0235](https://rustsec.org/advisories/RUSTSEC-2026-0235): insufficient validation can allow out-of-bounds reads for archived `Rc`/`Arc` unsized pointees. | Pulled by Wasmer 3.3.0 in SDK host tooling. Upstream fix is in rkyv 0.8.17, incompatible with the pinned Wasmer 3 series. |
| Medium | `tokio-tar 0.3.1`, [RUSTSEC-2025-0111](https://rustsec.org/advisories/RUSTSEC-2025-0111): malformed PAX headers can produce file-smuggling behavior; crate is archived. | Transitive through testcontainers and the SDK devnet environment. No fixed upstream version. |

The ten audit warnings were: unmaintained `derivative 2.2.0`, `fxhash 0.2.1`, `mach 0.3.2`, `paste 1.0.15`, `proc-macro-error 1.0.4`, and `rustls-pemfile 2.2.0`; unsoundness advisories for `lru 0.16.4`, `memmap2 0.5.10`, and `rand 0.8.5` / `rand 0.9.1`. These are also host/test transitive dependencies. They were not hidden with an audit ignore list.

### Existing audit findings rechecked

Every finding marked resolved in the v0.3.0 report was checked against current code and tests. All 17 remain fixed:

| v0.3.0 item | Check result |
| --- | --- |
| H-01 Bandersnatch/Jubjub parameters | Constants match the corrected curve values; parameter and curve tests pass. |
| M-01 ruint-to-Uint byte padding | Conversion pads to the fixed limb width; regression test passes. |
| M-02 `into_u128` limb check | One-limb values are handled without indexing a nonexistent second limb; edge tests pass. |
| M-03 `ExpandedSecretKey` zeroization | Secret state is non-`Copy` and uses zeroizing drop behavior; zeroization test passes. |
| M-04 P-256 signature malleability | P-256 wrapper rejects the high-s form and checks precompile inputs; precompile tests pass. |
| L-01 `AdminChanged` indexing | Both addresses are non-indexed as specified by EIP-1967. |
| L-02 `normalize_batch` with zero z | Zero z-coordinates are rejected before inversion. |
| L-03 UUPS implementation/context check | Proxy-only and UUID checks use the SDK VM context and implementation slot. |
| L-04 SafeERC20 `allowance` for no-code/fallback contracts | Typed contract call and return decoding reject missing/malformed allowance responses. |
| L-05 SafeERC20 relaxed ERC-1363 calls | High-level calls require a real contract and correctly decoded success. |
| N-01 Projective-to-affine zero z | Explicit zero-coordinate validation prevents inversion panic. |
| N-02 Enumerable range `values_slice` | Range API is implemented and tested. |
| N-03 EdDSA `compute_R` public key | Computation uses the same validated public key variable. |
| N-04 Scalar/WideScalar modulus parity | Compile-time parameter assertions and matching curve constants prevent divergence. |
| N-05 Enumerable gas warning | Unbounded enumeration methods document cost and scaling. |
| N-06 EdDSA byte-length typo | Error/doc text uses bytes rather than bits. |
| N-07 Compressed-point spare MSB | Compile-time field-width assertion reserves the sign bit. |

The v0.2.0 report's one High, two Medium, nine Low, and ten Notes were also checked. H-01 shift overflow; M-01 zero remainder; L-01 Merkle commutativity docs; L-02 carry/borrow types; L-03 `ct_num_bits`; L-04 hexadecimal overflow; L-05 wrapper decimals; L-06 read-only ERC-4626 trait methods; L-07 duplicate selectors; L-08 generated generic/unsafe trait components; L-09 constant-time big-integer comparisons; N-01 `must_use`; N-02 redundant attribute; N-03 inlining; N-04 projective equality comment; N-05 source attribution; N-06 Poseidon2 sponge documentation; N-07 shift semantics; N-08 interface-id documentation; and N-09 typos remain resolved in the current tree. N-10 coverage was acknowledged; current comparable core coverage is higher than v0.3.0 but is not 100%.

The v0.2.0 **M-02 custom selector/implementation mismatch** was acknowledged, not resolved. The macro can calculate an interface id for a declared selector but still cannot prove that a downstream implementation routes the same custom selector. The new collision checks do not close this gap.

The v0.2.0 Poseidon2 recommendations to validate S-box invertibility and full-round/security-minimum parameters are implemented and have negative regression tests.

The v0.1.0 report's ERC-165 support, checkpoint widths, metadata/URI behavior, ERC-20 validation, pausable exposure, TODO cleanup, Merkle hash swap, Pausable docs, and unused ECDSA error findings were checked and remain resolved. The v0.1.0 **L-05 burnable visibility** and **L-06 mixed composition styles** were acknowledged because the older SDK lacked a Solidity-like inheritance mechanism; this migration now uses explicit SDK traits but consumers still need to compose each public trait explicitly. v0.1.0 **N-02 mixed error types** was acknowledged and remains a design inconsistency across extensions. These are API consistency issues rather than known state-integrity vulnerabilities.

## Audit conclusion

The listed migration-time contract findings have regression PoCs and fixes, and the proxy-storage comparison found no reordered or retyped existing storage fields. The principal remaining risks are host-only SDK test-tool advisories, variable-time off-chain EdDSA signing, downstream custom-selector mismatch, and the documented need to propagate extension errors. Coverage improved over v0.3.0 but did not reach the 100% target; the exact figures and the LLVM/WASM instrumentation limitation are recorded in [MIGRATION.md](MIGRATION.md).
