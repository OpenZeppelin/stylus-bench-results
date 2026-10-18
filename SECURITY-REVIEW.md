# Security review

## Scope and method

Reviewed the v0.3.0 migration across `contracts`, `contracts-proc`, `lib/crypto`,
all examples, the SDK devnet E2E harness, benches, fuzz targets, and public
documentation. The review used source inspection, unit and E2E regression tests,
property tests, differential vectors against v0.3.0, fuzz smoke runs, storage
layout checks, and `cargo audit`. No mutation testing or formal verification
was used.

Every new finding below has a regression test committed before its fix. The
commit IDs identify the PoC test and the separate fix. The final workspace run
reported 738 passed, 0 failed, and 0 ignored unit/integration tests; the full
devnet matrix is recorded in `MIGRATION.md`.

## Security invariants

| Area | Invariants checked
|---|---|
| Ownable and AccessControl | Only the owner or the configured role admin can perform privileged actions. Ownership and role membership changes update the corresponding state and events together. Initialization is one-time; first-caller initialization must be atomic with proxy deployment or upgrade.
| ERC-20 and extensions | Transfers conserve balances; total supply changes only on authorized mint/burn; approvals are spent consistently; capped minting never exceeds the cap; pause, permit, and flash-mint paths preserve the same checks-effects-interactions rules. A caught error must not leave partial local writes.
| ERC-721 and extensions | Owner, balance, approval, and enumeration state agree, including token ID zero. Safe-mint callbacks observe fully updated state. Consecutive minting is restricted to its one-time constructor phase. Wrapper receipts remain backed by the underlying NFT and unauthorized callers cannot burn another owner's receipt.
| ERC-1155 and extensions | Per-ID and aggregate supply equal the balances; batch updates are consistent; burns require owner/operator authority; URI changes are privileged; receiver hooks return the required selector; callback errors roll back the transaction.
| ERC-2981 and ERC-4626 | Royalty amounts use the configured fee basis and full-precision multiplication. Vault conversions round in the documented direction and deposits/withdrawals preserve asset/share accounting; failed external token operations do not leave partial state.
| UUPS, ERC-1967, and beacon proxies | Upgrade entrypoints require the consumer's authority, proxy-only checks reject direct implementation calls, implementations have code and the expected UUID, versions do not regress, and initializer calldata is atomic with deployment/upgrade. Existing storage slots and field order remain compatible.
| Vesting wallet | Released amounts never exceed vested amounts or the allocation; ETH/ERC-20 release accounting is monotonic and updates only with successful transfers.
| ECDSA, EdDSA, and P-256 | Invalid encodings, malformed precompile output, non-canonical scalars, identity/torsion points, and invalid curve parameters cannot produce successful verification. Secret key material is zeroized on drop.
| Merkle, Poseidon, Pedersen, and field arithmetic | Hash/proof inputs follow the documented ordering; field values are reduced/canonical; limb operations do not overflow or index out of range; curve and hash parameter assumptions are explicit.
| Storage collections and macros | Lengths and indices stay in bounds, sentinel values cannot alias valid entries, generated interface selectors are unique within a trait, and ERC-165 IDs match the declared trait selectors. The remaining trait/implementation selector mismatch limitation is documented below.
| Examples | Every mint, pause, role-admin, upgrade, burn, URI, metadata, and fee-setting entrypoint has the stated authority. Constructor owners are explicit under SDK deployer semantics; proxy owner initialization is one-time and must be called atomically.

## New findings fixed in this migration

Severity is based on the effect if a consumer exposed the affected component or
example as written. Example-only findings are identified as such.

| ID | Severity and finding | Trigger and impact | PoC test commit | Fix commit(s) and status
|---|---|---|---|---|
| N-01 | Medium — ERC-721 owner enumeration treated token ID zero as the empty-slot sentinel. | Mint token ID zero and query owner enumeration; the owner index could be omitted or misread. Legacy v0.3.0 holdings also need a one-time enumeration backfill. | `9814668`, `d826127` | `47130e3`, `6ffc496`, storage-preserving follow-up `0dd9941`; fixed and documented in the upgrade guide.
| N-02 | High — EdDSA verification accepted identity or torsion points. | Supply identity/small-subgroup points in a signature/key pair; the verification equation could accept an invalid signature. | `6d66f62`, `2ef692c` | `d3b794e`, `4a35a8c`; rejects identity and validates subgroup membership.
| N-03 | Medium — ERC-721 consecutive batch mint could be invoked after construction. | Call the batch helper from a runtime entrypoint or during an unsafe upgrade initializer; runtime minting bypassed the intended consecutive-mint phase. | `67aa023`, `4b5e9ad`, `f16fd84` | `6f0d93d`, `0c3b7b4`, `9f23b9c`; one-time phase is closed before returning from construction and storage remains compatible.
| N-04 | High — token mutators could write state and then return `Err`. | A consumer catches the library error and returns success; because Stylus does not revert on a returned Rust error, earlier balance, allowance, permit, or supply writes could persist. | `65291a4`, `a8cf42b`, `cb37e81`, `e4756ea` | `2a55a64`; validates local failure conditions before committing local state and documents callback error propagation.
| N-05 | Medium — ERC-2981 default fee denominator could be zero. | Query royalties before explicitly setting the denominator; the default denominator yielded zero royalty despite the documented basis-point default. | `b2fe533` | `243c990`; zero stored denominator now uses 10,000 basis points.
| N-06 | Medium — the example UUPS `set_version` wrapper lacked owner authorization. | Any caller could advance proxy version state and interfere with the owner's upgrade flow. | `68c2a0f` | `dbc29e1`; wrapper checks `only_owner` before writing the version.
| N-07 | Medium — `ERC1155Supply` burn entrypoints did not check owner/operator approval. | Call `burn` or `burn_batch` for another account; balances and supply could be destroyed without authorization. | `7544138` | `ca4973e`; both entrypoints require the account or an approved operator.
| N-08 | Medium — ECDSA treated malformed/empty `ecrecover` output as the zero address. | A malformed precompile result could be interpreted as a recovered zero address, which is unsafe for callers that compare against a zero signer. | `34e3601` | `de34730`; malformed return lengths now return an error.
| N-09 | Medium — ERC-721 wrapper allowed unauthorized receipt burns and accepted calls to a no-code underlying address. | Burn another holder's receipt or configure a non-contract underlying; the receipt/collateral relation could be broken or an unbacked receipt created. | `5d05b00`, `1a6c83b`, `35605c9` | `bfd03de`, `8f3c5fc`; burn checks ownership/approval and deposit, withdrawal, and receiver hooks reject no-code underlyings.
| N-10 | Low — ERC-2981 royalty multiplication could overflow at valid large inputs. | Query royalties with a large sale price and numerator; intermediate multiplication reverted even when the final quotient fit. | `7075f77` | `8446db4`; uses full-precision multiplication and rounds down.
| N-11 | Medium — distinct interface signatures could collide on a four-byte selector. | Define colliding selectors in one `#[interface_id]` trait; router dispatch could make one function unreachable. | `77aeb3b` | `051760c`; compile-time selector collision checks reject the trait.
| N-12 | Medium — safe ERC-721 mint invoked the receiver before enumeration was complete. | A receiver reenters during safe mint and reads/transfers/burns the new token; owner enumeration could become inconsistent. | `b674a87`, `833a76a` | `7426e81`; enumeration is updated before the external receiver callback.
| N-13 | Medium — example mint and pause entrypoints lacked owner checks. | A non-owner called mint (or pause controls in ERC-20); sample deployments allowed arbitrary supply creation or service disruption. Affected examples: `basic/token`, `erc20`, `erc20-permit`, `erc1155`, `erc1155-supply`, `erc721`, `erc721-metadata`, `erc721-consecutive`, `erc20-flash-mint`, and both UUPS versions. | `3fc8f0f`, `e34f8f7`, `f5569bb`, `74ea448`, `9215928`, `e40b580`, `e98e3f1`, `3557b98`, `fea7115`, `58ae6d9`, `b13fce9` | `9e680b9`, `f905eb5`, `91e49d3`, `658c9ea`, `217d51e`, `8f4f1e1`, `3ab09e0`, `9f23b9c`, `3e78b21`, `188acfc`; owner checks and explicit owner initialization were added.
| N-14 | Medium — example NFT burn/metadata and ERC-1155 URI setters were callable without the intended authority. | A non-owner burned someone else's token or changed global token metadata/URI. | `1b259dd`, `6a4dd3a`, `ee10566`, `0a0a5fa` | `6638b13`, `8515451`, `7c52cb5`, `df4c7fc`; burn checks owner/approval and metadata/URI setters are owner-controlled.
| N-15 | Medium — example flash-mint fee and access-control role-admin helpers lacked authorization. | An arbitrary caller changed fees or the admin of privileged roles. | `55bdf07`, `8a57080` | `a326ef2`, `fabcd86`; setters require the owner/default admin.
| N-16 | Medium — beacon rollback example allowed unauthorized upgrades. | A caller changed the beacon implementation without owning the example proxy. | `a631b1c` | `4080346`; upgrade entrypoint checks the configured owner.
| N-17 | Low — constructor ownership assumptions differed under SDK deployment. | The E2E deployer contract, rather than the intended EOA, was observed as the constructor caller; example ownership could be assigned unexpectedly. | `a38c695`, `17e1d93` | `2114b84`; examples accept an explicit final `initial_owner` argument and the upgrade guide lists the new constructor ABI.

## Existing audit findings checked

The three PDFs in `audits/` were extracted and checked against current source,
tests, and the audit's resolution notes. “Resolved” below means the current
code still contains the claimed mitigation; it does not rely on the report's
status alone.

### v0.3.0 audit — 17/17 marked resolved

| Finding | Current check
|---|---|
| H-01 incorrect Bandersnatch/Jubjub parameters | Current curve parameter definitions and crypto tests use the corrected constants (`lib/crypto/src/curve/te/instance/`).
| M-01 inadequate byte padding in `ruint` conversion | Conversion code in `lib/crypto/src/arithmetic/uint.rs` uses explicit width-aware byte handling; conversion tests pass.
| M-02 incorrect limb check in `into_u128` | `into_u128` in `lib/crypto/src/arithmetic/uint.rs` handles the valid one- and two-limb cases; arithmetic tests pass.
| M-03 incomplete `ExpandedSecretKey` zeroization | `lib/crypto/src/eddsa.rs` derives drop zeroization and tests the zeroize-on-drop property.
| M-04 P-256 signature malleability | `contracts/src/utils/precompiles/p256_verify.rs` rejects high-s signatures; `test_p256_verify_rejects_higher_s` passes.
| L-01 incorrectly indexed `AdminChanged` event parameters | `contracts/src/proxy/erc1967/utils.rs` emits and tests the expected previous/new admin values.
| L-02 zero-`z` handling in `normalize_batch` | `lib/crypto/src/curve/sw/projective.rs` and `curve/te/projective.rs` handle the zero point consistently; edge tests pass.
| L-03 UUPS immutable implementation address check | `contracts/src/proxy/utils/uups_upgradeable.rs` compares implementation context and proxy slot in `only_proxy`/`not_delegated` checks.
| L-04 `SafeErc20::allowance` on no-code/fallback tokens | `contracts/src/token/erc20/utils/safe_erc20.rs` checks code and uses a validated high-level call.
| L-05 `*_relaxed` behavior differed from Solidity | The relaxed transfer/callback methods in `safe_erc20.rs` now preserve the documented return and callback behavior; corresponding tests pass.
| N-01 panic converting projective point with `z = 0` | Zero-projective conversion is tested and returns the documented identity/invalid result.
| N-02 missing ranged `EnumerableSet::values` | `values_slice(start, end)` is implemented in `contracts/src/utils/structs/enumerable_set/mod.rs`, with range/property tests.
| N-03 inconsistent EdDSA public-key use | Signing and verification derive/use the same public key representation in `lib/crypto/src/eddsa.rs` tests.
| N-04 missing Scalar/WideScalar equality checks | EdDSA scalar bounds and equality assumptions are checked before verification.
| N-05 enumerable gas warning | Rustdoc for enumerable operations documents their gas and unbounded-cost implications.
| N-06 EdDSA error-message typo | Current EdDSA error text is corrected.
| N-07 compressed-point `y` MSB validation | Compressed-point decoding validates the sign bit and field encoding; negative vectors pass.
### v0.2.0 audit — 20/22 marked resolved

| Finding | Audit status | Current verification
|---|---|---|
| H-01 limb shift overflow in `shr_assign`/`shl_assign` | Resolved | Current shifts use checked shift operations; boundary/property tests cover overflow.
| M-01 underflow in `ct_rem` | Resolved | The old `ct_rem` implementation is gone; current `WideUint::rem` handles zero dividend and has arithmetic edge tests.
| L-01 Merkle verification assumes commutative custom hashing | Resolved | `merkle.rs` documents the commutative requirement and uses `commutative_hash_pair`; `hash.rs` tests order independence.
| L-02 carry/borrow parameter types | Resolved | `adc`/`sbb` and their assignment variants consistently use `bool` carry/borrow values.
| L-03 ambiguous `ct_num_bits` encoding | Resolved | The current `num_bits` contract explicitly says zero needs one bit and includes examples/tests.
| L-04 overflow in `from_str_hex` | Resolved | The parser checks limb bounds and `from_str_hex_should_panic_on_overflow` covers the failure.
| L-05 ERC-20 wrapper underlying decimals | Resolved | `Erc20Wrapper::decimals` reads the underlying token metadata; wrapper tests cover the result.
| L-06 read-only `IErc4626` methods required `&mut self` | Resolved | Read-only methods now use `&self`; trait and implementations compile with that signature.
| L-07 custom selectors could create unreachable patterns | Resolved | `interface_id` rejects duplicate signatures and four-byte collisions at compile time.
| L-08 `interface_id` lost generic bounds/`unsafe` | Resolved | The proc macro preserves the original trait visibility, `unsafe`, generics, bounds, supertraits, and where clause.
| L-09 big integer comparisons could early-return | Resolved | The old early-exit comparison code is gone; current comparisons process the fixed limb sequence without early return. No separate timing analysis was performed.
| N-01 missing `must_use` on multi-proof verification | Resolved | Explicit `#[must_use]` attributes are now present on both multi-proof entrypoints in `lib/crypto/src/merkle.rs`.
| N-02 redundant `adc` lint attribute | Resolved | The redundant function-level cast lint attribute is absent.
| N-03 missing inlining annotations | Resolved | Small arithmetic and field helpers use the intended inline annotations.
| N-04 incorrect projective equality comment | Resolved | Current projective equality documentation matches the cross-multiplied implementation.
| N-05 source attribution | Resolved | License and attribution notices remain in source and crate documentation.
| N-06 documentation enhancements | Resolved | The crate and module docs cover the public crypto APIs and their assumptions.
| N-07 shift overflow behavior | Resolved | Shift overflow behavior is explicit and covered by boundary tests.
| N-08 `interface_id` documented as a constant | Resolved | The proc-macro docs describe the associated function actually generated.
| N-09 typographical errors | Resolved | The cited identifiers and comments are corrected.

The two items not marked resolved remain:

* **M-02 (Medium), custom selector mismatch between `#[interface_id]` and the
  `#[public]` implementation.** The proc macro cannot inspect a separately
  expanded implementation trait. A different custom selector can make the
  ERC-165 ID claim an interface whose selector is not routed. The migration
  rejects duplicate signatures and actual four-byte collisions within an
  interface, but does not solve this cross-macro mismatch. `contracts-proc`
  documentation and the upgrade guide require repeating the same selector
  override and checking the exported ABI. This is an existing audited
  integration risk, not a new finding.
* **N-10 (Note), incomplete unit-test coverage.** Coverage is higher than the
  v0.3.0 baseline (88.16% lines versus 82.61%), and additional property,
  differential, fuzz, and E2E tests were added. It is not 100%; see
  `MIGRATION.md` for the measurement limits and remaining uncovered code.

### v0.1.0 audit — acknowledged design items

| Finding | Audit status | Current verification
|---|---|---|
| M-01 ERC-721 did not support ERC-165 | Resolved | ERC-721 advertises and tests its supported interfaces.
| L-01 incomplete checkpoints implementation | Resolved | Checkpoint operations and boundary behavior are implemented and covered by tests.
| L-02 inconsistent ERC-721 extension behavior | Resolved | Shared ERC-721 update/authorization paths keep extension behavior consistent.
| L-03 missing ERC-20 validation checks | Resolved | Transfer, approval, and mint/burn paths validate zero addresses and balances.
| L-04 pause/unpause exposed without policy | Resolved | The component exposes internal operations; consuming examples apply owner authorization.
| N-01 TODO/FIXME cleanup | Resolved for the cited items | The cited metadata TODO is gone; remaining TODOs are reviewed as current maintenance notes.
| N-03 unnecessary swap in commutative hash | Resolved | `commutative_hash_pair` reverses the argument order directly.
| N-04 misleading Pausable documentation | Resolved | Pausable docs describe functions and explicit composition, not modifiers.
| N-05 unused error message | Resolved | The unused message is absent from current source.

These items were acknowledged as not fixed and remain design constraints:

* **L-05 (Low), burnable methods are not exposed automatically.** Extensions
  remain storage components; the consuming contract explicitly exposes and
  authorizes burn methods. SDK 0.10 supports composing multiple public blocks,
  and the examples show the explicit public interface pattern.
* **L-06 (Low), extension implementation styles are not uniform.** Rust
  components cannot use Solidity-style inheritance. The design notes explain
  explicit component composition and shared update paths.
* **N-02 (Note), error type style is not uniform.** Existing public error types
  are retained for ABI compatibility. The migration documents concrete ABI
  error types and conversion requirements rather than changing every
  extension's error surface.

## Dependency audit

The root `cargo audit` exits 1 with five advisories in the SDK devnet/test-tool
dependency graph, not in the contract WASM dependency graph: `brotli-sys`
RUSTSEC-2021-0131 (integer overflow in bundled Brotli), `hickory-proto`
RUSTSEC-2026-0119 (CPU exhaustion) and RUSTSEC-2026-0118 (unbounded NSEC3
validation loop), `rkyv` RUSTSEC-2026-0235 (out-of-bounds reads with
insufficient archive validation), and `tokio-tar` RUSTSEC-2025-0111 (PAX
header file smuggling). The available SDK `stylus-tools` 0.10.10 graph pins
these transitive versions; some advisories have no fixed release in that
graph. `bytes` RUSTSEC-2026-0007 was fixed by updating the root and template
lockfiles to bytes 1.11.1. The template and fuzz lockfile audits found no
vulnerabilities; they reported seven and three allowed unmaintained/unsound
dependency warnings respectively.

`cargo deny` could not provide a policy result because the repository has no
`deny.toml`; its default license check rejects dependencies without an explicit
policy. No dependency was silently accepted or suppressed by this review.

## Residual operational risks

`initializeOwner(address)` on the example proxy contracts is deliberately
first-caller-wins. A proxy upgrade must call it atomically with the upgrade;
leaving an uninitialized proxy externally callable transfers owner selection
to the first caller. Keep consumer-specific upgrade authorization on public
initializers and never expose a gap between upgrade and initialization.

The ERC-721 ID-zero enumeration compatibility fix cannot infer which legacy
owner-index slots previously held token ID zero. An existing proxy with such
holdings must run the documented one-time owner enumeration backfill before
relying on `tokenOfOwnerByIndex` for those holdings.
