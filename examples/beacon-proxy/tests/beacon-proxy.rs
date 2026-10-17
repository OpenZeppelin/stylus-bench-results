#![cfg(feature = "devnet")]

use abi::BeaconProxyExample;
use alloy::{
    primitives::{Address, U256},
    sol_types::{SolCall, SolError},
};
use eyre::Result;
use mock::{erc20, erc20::ERC20Mock, evm};
use stylus_sdk::abi::Bytes;
use stylus_test_support::{
    constructor, receipt, send, watch, Account, EventExt, Revert,
};

mod abi;
mod mock;

fn zero_bytes() -> Bytes {
    vec![].into()
}

const INITIAL_BALANCE: u64 = 37;

async fn deploy_proxy(
    account: &Account,
    implementation: Address,
) -> Result<(Address, Address)> {
    let beacon = account
        .as_deployer()
        .with_constructor(constructor!(implementation, account.address()))
        .with_example_name("upgradeable-beacon")
        .deploy()
        .await?
        .contract_address;
    let proxy_address = account
        .as_deployer()
        .with_build_feature("devnet")
        .with_constructor(constructor!(beacon, zero_bytes()))
        .with_example_name("beacon-proxy")
        .deploy()
        .await?
        .contract_address;
    let proxy = BeaconProxyExample::new(proxy_address, &account.wallet);
    watch!(proxy.mint(account.address(), U256::from(INITIAL_BALANCE)))?;

    Ok((beacon, proxy_address))
}

async fn assert_proxy_state(
    proxy_address: Address,
    account: &Account,
    beacon: Address,
    implementation: Address,
) -> Result<()> {
    let proxy = BeaconProxyExample::new(proxy_address, &account.wallet);
    assert_eq!(proxy.getBeacon().call().await?, beacon);
    assert_eq!(proxy.implementation().call().await?, implementation);
    assert_eq!(
        proxy.balanceOf(account.address()).call().await?,
        U256::from(INITIAL_BALANCE)
    );
    assert_eq!(proxy.totalSupply().call().await?, U256::from(INITIAL_BALANCE));
    Ok(())
}

async fn mine_failed_upgrade(
    proxy_address: Address,
    account: &Account,
    beacon: Address,
    data: Bytes,
) -> Result<()> {
    let proxy = BeaconProxyExample::new(proxy_address, &account.wallet);
    let receipt = receipt!(proxy
        .testUpgradeBeaconToAndCall(beacon, data)
        .gas(3_000_000))?;
    assert!(
        !receipt.status(),
        "failed beacon upgrade must be mined as reverted"
    );
    Ok(())
}

#[tokio::test]
async fn constructs() -> Result<()> {
    let alice = Account::new().await?;

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
    let alice = Account::new().await?;

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
async fn fallback() -> Result<()> {
    let alice = Account::new().await?;
    let bob = Account::new().await?;

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
    let alice = Account::new().await?;
    let bob = Account::new().await?;

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

#[tokio::test]
async fn upgrade_beacon_to_and_call_reverts_for_invalid_implementation_and_rolls_back(
) -> Result<()> {
    let alice = Account::new().await?;
    let old_implementation = erc20::deploy(&alice.wallet).await?;
    let (old_beacon, proxy_address) =
        deploy_proxy(&alice, old_implementation).await?;
    let proxy = BeaconProxyExample::new(proxy_address, &alice.wallet);
    let invalid_beacon =
        evm::deploy_constant_beacon(&alice.wallet, alice.address()).await?;
    let data: Bytes = vec![0xde, 0xad, 0xbe, 0xef].into();

    let Err(error) = proxy
        .testUpgradeBeaconToAndCall(invalid_beacon, data.clone())
        .call()
        .await
    else {
        panic!("beacon implementation without code must be rejected");
    };
    assert!(error.reverted_with(
        BeaconProxyExample::ERC1967InvalidImplementation {
            implementation: alice.address(),
        }
    ));

    mine_failed_upgrade(proxy_address, &alice, invalid_beacon, data).await?;
    assert_proxy_state(proxy_address, &alice, old_beacon, old_implementation)
        .await?;
    Ok(())
}

#[tokio::test]
async fn upgrade_beacon_to_and_call_reverts_for_zero_implementation_and_rolls_back(
) -> Result<()> {
    let alice = Account::new().await?;
    let old_implementation = erc20::deploy(&alice.wallet).await?;
    let (old_beacon, proxy_address) =
        deploy_proxy(&alice, old_implementation).await?;
    let proxy = BeaconProxyExample::new(proxy_address, &alice.wallet);
    let zero_implementation_beacon =
        evm::deploy_constant_beacon(&alice.wallet, Address::ZERO).await?;
    let data: Bytes = vec![0xde, 0xad, 0xbe, 0xef].into();

    let Err(error) = proxy
        .testUpgradeBeaconToAndCall(zero_implementation_beacon, data.clone())
        .call()
        .await
    else {
        panic!("beacon returning the zero address must be rejected");
    };
    assert!(error.reverted_with(
        BeaconProxyExample::ERC1967InvalidImplementation {
            implementation: Address::ZERO,
        }
    ));

    mine_failed_upgrade(
        proxy_address,
        &alice,
        zero_implementation_beacon,
        data,
    )
    .await?;
    assert_proxy_state(proxy_address, &alice, old_beacon, old_implementation)
        .await?;
    Ok(())
}

#[tokio::test]
async fn upgrade_beacon_to_and_call_reverts_on_empty_delegatecall_and_rolls_back(
) -> Result<()> {
    let alice = Account::new().await?;
    let old_implementation = erc20::deploy(&alice.wallet).await?;
    let (old_beacon, proxy_address) =
        deploy_proxy(&alice, old_implementation).await?;
    let proxy = BeaconProxyExample::new(proxy_address, &alice.wallet);
    let empty_reverter = evm::deploy_empty_reverter(&alice.wallet).await?;
    let reverting_beacon =
        evm::deploy_constant_beacon(&alice.wallet, empty_reverter).await?;
    let data: Bytes = vec![0xde, 0xad, 0xbe, 0xef].into();

    let Err(error) = proxy
        .testUpgradeBeaconToAndCall(reverting_beacon, data.clone())
        .call()
        .await
    else {
        panic!("empty delegatecall revert must fail the upgrade");
    };
    assert!(error.reverted_with(BeaconProxyExample::FailedCall {}));

    mine_failed_upgrade(proxy_address, &alice, reverting_beacon, data).await?;
    assert_proxy_state(proxy_address, &alice, old_beacon, old_implementation)
        .await?;
    Ok(())
}

#[tokio::test]
async fn upgrade_beacon_to_and_call_bubbles_implementation_revert_and_rolls_back(
) -> Result<()> {
    let alice = Account::new().await?;
    let bob = Account::new().await?;
    let old_implementation = erc20::deploy(&alice.wallet).await?;
    let (old_beacon, proxy_address) =
        deploy_proxy(&alice, old_implementation).await?;
    let proxy = BeaconProxyExample::new(proxy_address, &alice.wallet);
    let reverting_implementation = erc20::deploy(&alice.wallet).await?;
    let reverting_beacon =
        evm::deploy_constant_beacon(&alice.wallet, reverting_implementation)
            .await?;
    let amount = U256::from(INITIAL_BALANCE + 1);
    let data: Bytes =
        BeaconProxyExample::transferCall { recipient: bob.address(), amount }
            .abi_encode()
            .into();

    let Err(error) = proxy
        .testUpgradeBeaconToAndCall(reverting_beacon, data.clone())
        .call()
        .await
    else {
        panic!("reverting implementation initialization must fail");
    };
    let reason = BeaconProxyExample::ERC20InsufficientBalance {
        sender: alice.address(),
        balance: U256::from(INITIAL_BALANCE),
        needed: amount,
    }
    .abi_encode();
    assert!(error.reverted_with(BeaconProxyExample::FailedCallWithReason {
        reason: reason.into()
    }));

    mine_failed_upgrade(proxy_address, &alice, reverting_beacon, data).await?;
    assert_proxy_state(proxy_address, &alice, old_beacon, old_implementation)
        .await?;
    Ok(())
}
