#![cfg(feature = "integration-tests")]

#[path = "../../test-support/mod.rs"]
mod test_support;
use crate::test_support::{
    Account, ContractInitializationError, EventExt, Revert, constructor,
    receipt, send, watch,
};
use abi::BeaconProxyExample;
use alloy::{primitives::U256, sol_types::SolCall};
use eyre::Result;
use mock::{erc20, erc20::ERC20Mock};
use stylus_sdk::{abi::Bytes, function_selector};

mod abi;
mod mock;

fn zero_bytes() -> Bytes {
    vec![].into()
}

#[tokio::test]
async fn constructs() -> Result<()> {
    let _test_context = crate::test_support::TestContext::new().await?;
    let alice = crate::test_support::Account::new().await?;

    let implementation_addr = erc20::deploy(&alice.wallet).await?;
    let beacon_addr = alice
        .as_deployer()
        .with_constructor(constructor!(implementation_addr, alice.address()))
        .with_example_name("upgradeable-beacon")
        .deploy()
        .await?
        .contract_address;
    let contract_addr = alice
        .as_deployer()
        .with_constructor(constructor!(beacon_addr, zero_bytes()))
        .deploy()
        .await?
        .contract_address;
    let contract = BeaconProxyExample::new(contract_addr, &alice.wallet);

    let implementation = contract.implementation().call().await?;
    assert_eq!(implementation, implementation_addr);

    let beacon = contract.getBeacon().call().await?;
    assert_eq!(beacon, beacon_addr);

    Ok(())
}

#[tokio::test]
async fn constructs_with_data() -> Result<()> {
    let _test_context = crate::test_support::TestContext::new().await?;
    let alice = crate::test_support::Account::new().await?;

    let implementation_addr = erc20::deploy(&alice.wallet).await?;
    let beacon_addr = alice
        .as_deployer()
        .with_constructor(constructor!(implementation_addr, alice.address()))
        .with_example_name("upgradeable-beacon")
        .deploy()
        .await?
        .contract_address;

    // mint 1000 tokens.
    let amount = U256::from(1000);

    let data = ERC20Mock::mintCall { account: alice.address(), value: amount };
    let data: Bytes = data.abi_encode().into();

    let contract_addr = alice
        .as_deployer()
        .with_constructor(constructor!(beacon_addr, data.clone()))
        .deploy()
        .await?
        .contract_address;
    let contract = BeaconProxyExample::new(contract_addr, &alice.wallet);

    let implementation = contract.implementation().call().await?;
    assert_eq!(implementation, implementation_addr);

    let beacon = contract.getBeacon().call().await?;
    assert_eq!(beacon, beacon_addr);

    // check that the balance can be accurately fetched through the proxy.
    let balance = contract.balanceOf(alice.address()).call().await?;
    assert_eq!(balance, amount);

    let total_supply = contract.totalSupply().call().await?;
    assert_eq!(total_supply, amount);

    Ok(())
}

#[tokio::test]
async fn constructor_rejects_invalid_beacon() -> Result<()> {
    let _test_context = crate::test_support::TestContext::new().await?;
    let alice = crate::test_support::Account::new().await?;

    let err = alice
        .as_deployer()
        .with_constructor(constructor!(alice.address(), zero_bytes()))
        .deploy()
        .await
        .expect_err("should reject an address without beacon code");
    assert!(err.downcast_ref::<ContractInitializationError>().is_some());

    Ok(())
}

#[tokio::test]
async fn constructor_reverts_when_initializer_reverts() -> Result<()> {
    let _test_context = crate::test_support::TestContext::new().await?;
    let alice = crate::test_support::Account::new().await?;

    let implementation_addr = erc20::deploy(&alice.wallet).await?;
    let beacon_addr = alice
        .as_deployer()
        .with_constructor(constructor!(implementation_addr, alice.address()))
        .with_example_name("upgradeable-beacon")
        .deploy()
        .await?
        .contract_address;
    let invalid_data: Bytes =
        function_selector!("nonExistentFunction").to_vec().into();

    let err = alice
        .as_deployer()
        .with_constructor(constructor!(beacon_addr, invalid_data))
        .deploy()
        .await
        .expect_err("should bubble a reverting initializer");
    assert!(err.downcast_ref::<ContractInitializationError>().is_some());

    Ok(())
}

#[tokio::test]
async fn fallback() -> Result<()> {
    let _test_context = crate::test_support::TestContext::new().await?;
    let alice = crate::test_support::Account::new().await?;
    let bob = crate::test_support::Account::new().await?;

    let implementation_addr = erc20::deploy(&alice.wallet).await?;
    let beacon_addr = alice
        .as_deployer()
        .with_constructor(constructor!(implementation_addr, alice.address()))
        .with_example_name("upgradeable-beacon")
        .deploy()
        .await?
        .contract_address;
    let contract_addr = alice
        .as_deployer()
        .with_constructor(constructor!(beacon_addr, zero_bytes()))
        .deploy()
        .await?
        .contract_address;
    let contract = BeaconProxyExample::new(contract_addr, &alice.wallet);

    // verify initial balance is [`U256::ZERO`].
    let balance = contract.balanceOf(alice.address()).call().await?;
    assert_eq!(balance, U256::ZERO);

    let total_supply = contract.totalSupply().call().await?;
    assert_eq!(total_supply, U256::ZERO);

    // mint 1000 tokens.
    let amount = U256::from(1000);
    watch!(contract.mint(alice.address(), amount))?;

    // check that the balance can be accurately fetched through the proxy.
    let balance = contract.balanceOf(alice.address()).call().await?;
    assert_eq!(balance, amount);

    let total_supply = contract.totalSupply().call().await?;
    assert_eq!(total_supply, amount);

    // check that the balance can be transferred through the proxy.
    let receipt = receipt!(contract.transfer(bob.address(), amount))?;

    assert!(receipt.emits(BeaconProxyExample::Transfer {
        from: alice.address(),
        to: bob.address(),
        value: amount,
    }));

    let balance = contract.balanceOf(alice.address()).call().await?;
    assert_eq!(balance, U256::ZERO);

    let balance = contract.balanceOf(bob.address()).call().await?;
    assert_eq!(balance, amount);

    let total_supply = contract.totalSupply().call().await?;
    assert_eq!(total_supply, amount);

    Ok(())
}

#[tokio::test]
async fn fallback_returns_error() -> Result<()> {
    let _test_context = crate::test_support::TestContext::new().await?;
    let alice = crate::test_support::Account::new().await?;
    let bob = crate::test_support::Account::new().await?;

    let implementation_addr = erc20::deploy(&alice.wallet).await?;
    let beacon_addr = alice
        .as_deployer()
        .with_constructor(constructor!(implementation_addr, alice.address()))
        .with_example_name("upgradeable-beacon")
        .deploy()
        .await?
        .contract_address;
    let contract_addr = alice
        .as_deployer()
        .with_constructor(constructor!(beacon_addr, zero_bytes()))
        .deploy()
        .await?
        .contract_address;
    let contract = BeaconProxyExample::new(contract_addr, &alice.wallet);

    let invalid_amount = U256::from(1000);

    let err = send!(contract.transfer(bob.address(), invalid_amount))
        .expect_err("should revert");
    assert!(err.reverted_with(BeaconProxyExample::ERC20InsufficientBalance {
        sender: alice.address(),
        balance: U256::ZERO,
        needed: invalid_amount,
    }),);

    Ok(())
}
