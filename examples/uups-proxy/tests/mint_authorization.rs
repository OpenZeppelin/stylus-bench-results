#![cfg(feature = "e2e")]

#[path = "../../devnet_test_support.rs"]
mod devnet_test_support;

use crate::devnet_test_support::{constructor, send, watch, Account, Revert};
use abi::{Erc1967Example, UUPSProxyErc20Example};
use alloy::{
    primitives::{Address, Bytes, U256},
    sol_types::SolCall,
};
use eyre::Result;

#[path = "abi/mod.rs"]
mod abi;

async fn deploy_v1_proxy(alice: &Account) -> Result<Address> {
    let implementation = alice
        .as_deployer()
        .with_constructor(constructor!(alice.address()))
        .deploy()
        .await?
        .contract_address;
    let initialization: Bytes =
        UUPSProxyErc20Example::initializeCall { owner: alice.address() }
            .abi_encode()
            .into();
    let proxy = alice
        .as_deployer()
        .with_example_name("erc1967")
        .with_constructor(constructor!(implementation, initialization))
        .deploy()
        .await?
        .contract_address;

    Ok(proxy)
}

#[tokio::test]
async fn non_owner_cannot_mint_through_v1_proxy() -> Result<()> {
    let alice = Account::new().await?;
    let bob = Account::new().await?;
    let proxy_address = deploy_v1_proxy(&alice).await?;
    let proxy = Erc1967Example::new(proxy_address, &bob.wallet);

    let error = send!(proxy.mint(bob.address(), U256::from(1)))
        .expect_err("a non-owner must not mint through the V1 proxy");
    assert!(error.reverted_with(Erc1967Example::OwnableUnauthorizedAccount {
        account: bob.address()
    }));
    assert_eq!(U256::ZERO, proxy.totalSupply().call().await?);
    assert_eq!(U256::ZERO, proxy.balanceOf(bob.address()).call().await?);

    Ok(())
}

#[tokio::test]
async fn non_owner_cannot_mint_through_v2_proxy() -> Result<()> {
    let alice = Account::new().await?;
    let bob = Account::new().await?;
    let proxy_address = deploy_v1_proxy(&alice).await?;
    let owner_proxy = Erc1967Example::new(proxy_address, &alice.wallet);
    let v2_implementation = alice
        .as_deployer()
        .with_example_name("uups-proxy-new-version")
        .with_constructor(constructor!(alice.address()))
        .deploy()
        .await?
        .contract_address;

    watch!(owner_proxy.upgradeToAndCall(v2_implementation, vec![].into()))?;

    let attacker_proxy = Erc1967Example::new(proxy_address, &bob.wallet);
    let error = send!(attacker_proxy.mint(bob.address(), U256::from(1)))
        .expect_err("a non-owner must not mint through the V2 proxy");
    assert!(error.reverted_with(Erc1967Example::OwnableUnauthorizedAccount {
        account: bob.address()
    }));
    assert_eq!(U256::ZERO, attacker_proxy.totalSupply().call().await?);
    assert_eq!(
        U256::ZERO,
        attacker_proxy.balanceOf(bob.address()).call().await?
    );

    Ok(())
}
