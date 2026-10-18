# OpenZeppelin Contracts for Stylus

**A secure, modular smart contract library** for [Arbitrum Stylus](https://docs.arbitrum.io/stylus/gentle-introduction), written in Rust and inspired by [OpenZeppelin Contracts](https://github.com/OpenZeppelin/openzeppelin-contracts).

Stylus enables high-performance smart contracts in Rust, compiled to WebAssembly (Wasm), for deployment on Arbitrum chains.

## ✨ Features

- Security-first contracts, ported from the [`openzeppelin-contracts`] library.
- Full support for `no_std` Rust environments.
- Ready-to-use [SDK unit testing] and [local devnet testing] workflows, used in our own tests.
- Familiar, well-documented contract APIs.

[`openzeppelin-contracts`]: https://github.com/OpenZeppelin/openzeppelin-contracts
[SDK unit testing]: https://docs.rs/stylus-test/0.10.10/stylus_test/vm/struct.TestVM.html
[local devnet testing]: https://developer.arbitrum.io/stylus/fundamentals/testing-contracts

## 🚀 Usage

Add the crate to your Cargo.toml:

```toml
[dependencies]
# We recommend pinning to a specific version.
openzeppelin-stylus = "=0.3.0"
```

**Enable ABI export support**:

```toml
[features]
# stylus-sdk/export-abi will be enabled automatically.
export-abi = ["openzeppelin-stylus/export-abi"]
```

If you need typed Rust clients for calls to this contract from another Rust
contract, enable `contract-client-gen` on the contract package.

## 🧱 `no_std` Projects

This library is designed for `no_std` environments to reduce Wasm size.
Ensure your dependencies also avoid the standard library:

> ```toml
> [dependencies]
> alloy-primitives = { version = "=1.5.7", default-features = false }
> stylus-sdk = "=0.10.10"
> ```

## 🦀 Stable Rust & WASM Builds

This release uses stable Rust 1.91.0 and Stylus SDK 0.10.10. Install the `wasm32-unknown-unknown` target before building contracts.

The example contracts pin Binaryen 116 and the optimizer recipe in `Stylus.toml`. `cargo-stylus` applies this recipe during deployment and verification.

To compile contracts for Arbitrum Stylus, run:

```sh
cargo build --release --target wasm32-unknown-unknown
```

## 🧪 Example: ERC20 Token

Once defined as a dependency, use one of our pre-defined implementations by
importing them:

```rust
use openzeppelin_stylus::token::erc20::{self, Erc20, IErc20};
use stylus_sdk::{
    alloy_primitives::{Address, U256},
    prelude::*,
};

#[entrypoint]
#[storage]
struct Erc20Example {
    erc20: Erc20,
}

#[public]
#[implements(IErc20<Error = erc20::Error>)]
impl Erc20Example {}

#[public]
impl IErc20 for Erc20Example {
    type Error = erc20::Error;

    fn total_supply(&self) -> U256 {
        self.erc20.total_supply()
    }

    fn balance_of(&self, account: Address) -> U256 {
        self.erc20.balance_of(account)
    }

    fn transfer(
        &mut self,
        to: Address,
        value: U256,
    ) -> Result<bool, erc20::Error> {
        self.erc20.transfer(to, value)
    }

    fn allowance(&self, owner: Address, spender: Address) -> U256 {
        self.erc20.allowance(owner, spender)
    }

    fn approve(
        &mut self,
        spender: Address,
        value: U256,
    ) -> Result<bool, Self::Error> {
        self.erc20.approve(spender, value)
    }

    fn transfer_from(
        &mut self,
        from: Address,
        to: Address,
        value: U256,
    ) -> Result<bool, erc20::Error> {
        self.erc20.transfer_from(from, to, value)
    }
}
```

For a working demo with deployment and queries, check out the [basic] example.

Run the Stylus SDK unit tests with `cargo test --workspace`. Run the local
devnet integration suite with `./scripts/e2e-tests.sh`; this starts the SDK's
devnet unless `RPC_URL` points to a node you started yourself.

When upgrading an existing v0.3.0 contract, see the
[upgrade guide](docs/modules/ROOT/pages/upgrade-from-v0.3.0.adoc).

## 📚 Resources

- **Examples**: Browse our [examples] folder for full project templates.
- **Test helpers**: See [unit] and [integration] testing utilities.
- **Roadmap**: Follow planned features and modules in our [roadmap].

[basic]: ./examples/basic
[examples]: ./examples
[roadmap]: https://github.com/orgs/OpenZeppelin/projects/35/views/9

## 🤝 Contribute

OpenZeppelin Contracts for Stylus exists thanks to its contributors. There are
many ways you can participate and help build high-quality software. Check out
the [contribution guide](CONTRIBUTING.md)!

## 🔐 Security

- **Past audits**: See the [`audits`](./audits) folder.
- Refer to our [Security Policy](SECURITY.md) for guidelines on reporting vulnerabilities.

## ⚖️ License

OpenZeppelin Contracts for Stylus is released under
the [MIT License](./LICENSE).
