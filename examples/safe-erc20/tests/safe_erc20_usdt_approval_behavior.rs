#![cfg(feature = "e2e")]
#[path = "../../devnet_test_support.rs"]
mod devnet_test_support;

use crate::devnet_test_support::{receipt, watch, Account, EventExt};
use abi::{Erc20, SafeErc20};
use alloy::primitives::uint;
use mock::{erc20_force_approve, erc20_force_approve::ERC20ForceApproveMock};

mod abi;
mod mock;

#[tokio::test]
async fn safe_increase_allowance_works() -> eyre::Result<()> {
    let alice = Account::new().await?;
    let bob = Account::new().await?;

    let safe_erc20_addr = alice.as_deployer().deploy().await?.contract_address;
    let safe_erc20_alice = SafeErc20::new(safe_erc20_addr, &alice.wallet);
    let bob_addr = bob.address();

    let erc20_address = erc20_force_approve::deploy(&alice.wallet).await?;
    let erc20_alice = ERC20ForceApproveMock::new(erc20_address, &alice.wallet);

    let init_approval = uint!(100_U256);
    let value = uint!(10_U256);

    watch!(erc20_alice.forceApprove(safe_erc20_addr, bob_addr, init_approval))?;

    let initial_bob_allowance =
        erc20_alice.allowance(safe_erc20_addr, bob_addr).call().await?;
    assert_eq!(initial_bob_allowance, init_approval);

    let receipt = receipt!(safe_erc20_alice.safeIncreaseAllowance(
        erc20_address,
        bob_addr,
        value
    ))?;

    assert!(receipt.emits(Erc20::Approval {
        owner: safe_erc20_addr,
        spender: bob_addr,
        value: init_approval + value,
    }));

    let bob_allowance =
        erc20_alice.allowance(safe_erc20_addr, bob_addr).call().await?;
    assert_eq!(bob_allowance, init_approval + value);

    Ok(())
}

#[tokio::test]
async fn safe_decrease_allowance_works() -> eyre::Result<()> {
    let alice = Account::new().await?;
    let bob = Account::new().await?;

    let safe_erc20_addr = alice.as_deployer().deploy().await?.contract_address;
    let safe_erc20_alice = SafeErc20::new(safe_erc20_addr, &alice.wallet);
    let bob_addr = bob.address();

    let erc20_address = erc20_force_approve::deploy(&alice.wallet).await?;
    let erc20_alice = ERC20ForceApproveMock::new(erc20_address, &alice.wallet);

    let init_approval = uint!(100_U256);
    let value = uint!(10_U256);

    watch!(erc20_alice.forceApprove(safe_erc20_addr, bob_addr, init_approval))?;

    let initial_bob_allowance =
        erc20_alice.allowance(safe_erc20_addr, bob_addr).call().await?;
    assert_eq!(initial_bob_allowance, init_approval);

    let receipt = receipt!(safe_erc20_alice.safeDecreaseAllowance(
        erc20_address,
        bob_addr,
        value
    ))?;

    assert!(receipt.emits(Erc20::Approval {
        owner: safe_erc20_addr,
        spender: bob_addr,
        value: init_approval - value,
    }));

    let bob_allowance =
        erc20_alice.allowance(safe_erc20_addr, bob_addr).call().await?;
    assert_eq!(bob_allowance, init_approval - value);

    Ok(())
}

#[tokio::test]
async fn force_approve_works() -> eyre::Result<()> {
    let alice = Account::new().await?;
    let bob = Account::new().await?;

    let safe_erc20_addr = alice.as_deployer().deploy().await?.contract_address;
    let safe_erc20_alice = SafeErc20::new(safe_erc20_addr, &alice.wallet);
    let bob_addr = bob.address();

    let erc20_address = erc20_force_approve::deploy(&alice.wallet).await?;
    let erc20_alice = ERC20ForceApproveMock::new(erc20_address, &alice.wallet);

    let init_approval = uint!(100_U256);
    let updated_approval = uint!(10_U256);

    watch!(erc20_alice.forceApprove(safe_erc20_addr, bob_addr, init_approval))?;

    let initial_bob_allowance =
        erc20_alice.allowance(safe_erc20_addr, bob_addr).call().await?;
    assert_eq!(initial_bob_allowance, init_approval);

    let receipt = receipt!(safe_erc20_alice.forceApprove(
        erc20_address,
        bob_addr,
        updated_approval
    ))?;

    assert!(receipt.emits(Erc20::Approval {
        owner: safe_erc20_addr,
        spender: bob_addr,
        value: updated_approval,
    }));

    let bob_allowance =
        erc20_alice.allowance(safe_erc20_addr, bob_addr).call().await?;
    assert_eq!(bob_allowance, updated_approval);

    Ok(())
}
