#![cfg(feature = "e2e")]
#[path = "../../devnet_test_support.rs"]
mod devnet_test_support;

use crate::devnet_test_support::{
    constructor, receipt, send, Account, Revert,
};
use abi::Erc1967Example;
use alloy::primitives::Bytes;
use eyre::Result;

#[path = "abi/mod.rs"]
mod abi;

#[tokio::test]
async fn permissionless_version_write_cannot_block_initialization() -> Result<()>
{
    let alice = Account::new().await?;
    let bob = Account::new().await?;

    let logic_address = alice
        .as_deployer()
        .with_constructor(constructor!(alice.address()))
        .deploy()
        .await?
        .contract_address;

    let proxy_address = alice
        .as_deployer()
        .with_example_name("erc1967")
        .with_constructor(constructor!(logic_address, Bytes::new()))
        .deploy()
        .await?
        .contract_address;

    let attacker_proxy = Erc1967Example::new(proxy_address, &bob.wallet);
    let version_error = send!(attacker_proxy.setVersion())
        .expect_err("a non-owner must not write the proxy version");
    assert!(version_error.reverted_with(
        Erc1967Example::OwnableUnauthorizedAccount { account: bob.address() }
    ));

    let owner_proxy = Erc1967Example::new(proxy_address, &alice.wallet);
    let initialization = receipt!(owner_proxy.initialize(alice.address()));
    assert!(
        initialization.is_ok(),
        "a permissionless setVersion call on an uninitialized proxy must not permanently block its owner"
    );
    assert_eq!(alice.address(), owner_proxy.owner().call().await?);

    Ok(())
}
