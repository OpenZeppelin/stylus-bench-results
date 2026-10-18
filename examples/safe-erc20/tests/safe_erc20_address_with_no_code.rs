#![cfg(feature = "integration-tests")]

#[path = "../../test-support/mod.rs"]
mod test_support;
use crate::test_support::{receipt, send, Account, EventExt, Revert};
use abi::SafeErc20;
use alloy::primitives::{uint, U256};

mod abi;
mod mock;

#[tokio::test]
async fn reverts_on_transfer() -> eyre::Result<()> {
    let _test_context = crate::test_support::TestContext::new().await?;
    let alice = crate::test_support::Account::new().await?;
    let bob = crate::test_support::Account::new().await?;
    let has_no_code = crate::test_support::Account::new().await?;

    let safe_erc20_addr =
        alice.as_deployer_with_initial_owner().deploy().await?.contract_address;
    let safe_erc20_alice = SafeErc20::new(safe_erc20_addr, &alice.wallet);
    let bob_addr = bob.address();
    let has_no_code_addr = has_no_code.address();

    let value = uint!(1_U256);

    let err =
        send!(safe_erc20_alice.safeTransfer(has_no_code_addr, bob_addr, value))
            .expect_err("should not be able to invoke 'transfer' on EOA");
    assert!(err.reverted_with(SafeErc20::SafeErc20FailedOperation {
        token: has_no_code_addr
    }));

    Ok(())
}

#[tokio::test]
async fn returns_false_on_try_safe_transfer() -> eyre::Result<()> {
    let _test_context = crate::test_support::TestContext::new().await?;
    let alice = crate::test_support::Account::new().await?;
    let bob = crate::test_support::Account::new().await?;
    let has_no_code = crate::test_support::Account::new().await?;

    let safe_erc20_addr =
        alice.as_deployer_with_initial_owner().deploy().await?.contract_address;
    let safe_erc20_alice = SafeErc20::new(safe_erc20_addr, &alice.wallet);
    let bob_addr = bob.address();
    let has_no_code_addr = has_no_code.address();

    let value = uint!(0_U256);

    let receipt = receipt!(safe_erc20_alice.trySafeTransfer(
        has_no_code_addr,
        bob_addr,
        value
    ))?;

    assert!(receipt.emits(SafeErc20::False {}));

    Ok(())
}

#[tokio::test]
async fn reverts_on_transfer_from() -> eyre::Result<()> {
    let _test_context = crate::test_support::TestContext::new().await?;
    let alice = crate::test_support::Account::new().await?;
    let bob = crate::test_support::Account::new().await?;
    let has_no_code = crate::test_support::Account::new().await?;

    let safe_erc20_addr =
        alice.as_deployer_with_initial_owner().deploy().await?.contract_address;
    let safe_erc20_alice = SafeErc20::new(safe_erc20_addr, &alice.wallet);
    let alice_addr = alice.address();
    let bob_addr = bob.address();
    let has_no_code_addr = has_no_code.address();

    let value = uint!(1_U256);

    let err = send!(safe_erc20_alice.safeTransferFrom(
        has_no_code_addr,
        alice_addr,
        bob_addr,
        value
    ))
    .expect_err("should not be able to invoke 'transferFrom' on EOA");
    assert!(err.reverted_with(SafeErc20::SafeErc20FailedOperation {
        token: has_no_code_addr
    }));

    Ok(())
}

#[tokio::test]
async fn returns_false_on_try_safe_transfer_from() -> eyre::Result<()> {
    let _test_context = crate::test_support::TestContext::new().await?;
    let alice = crate::test_support::Account::new().await?;
    let bob = crate::test_support::Account::new().await?;
    let has_no_code = crate::test_support::Account::new().await?;

    let safe_erc20_addr =
        alice.as_deployer_with_initial_owner().deploy().await?.contract_address;
    let safe_erc20_alice = SafeErc20::new(safe_erc20_addr, &alice.wallet);
    let alice_addr = alice.address();
    let bob_addr = bob.address();
    let has_no_code_addr = has_no_code.address();

    let value = uint!(0_U256);

    let receipt = receipt!(safe_erc20_alice.trySafeTransferFrom(
        has_no_code_addr,
        alice_addr,
        bob_addr,
        value
    ))?;

    assert!(receipt.emits(SafeErc20::False {}));

    Ok(())
}

#[tokio::test]
async fn reverts_on_increase_allowance() -> eyre::Result<()> {
    let _test_context = crate::test_support::TestContext::new().await?;
    let alice = crate::test_support::Account::new().await?;
    let bob = crate::test_support::Account::new().await?;
    let has_no_code = crate::test_support::Account::new().await?;

    let safe_erc20_addr =
        alice.as_deployer_with_initial_owner().deploy().await?.contract_address;
    let safe_erc20_alice = SafeErc20::new(safe_erc20_addr, &alice.wallet);
    let bob_addr = bob.address();
    let has_no_code_addr = has_no_code.address();

    let value = uint!(1_U256);

    let err = send!(safe_erc20_alice.safeIncreaseAllowance(
        has_no_code_addr,
        bob_addr,
        value
    ))
    .expect_err("should not be able to invoke 'increaseAllowance' on EOA");
    assert!(err.reverted_with(SafeErc20::SafeErc20FailedOperation {
        token: has_no_code_addr
    }));

    Ok(())
}

#[tokio::test]
async fn reverts_on_decrease_allowance() -> eyre::Result<()> {
    let _test_context = crate::test_support::TestContext::new().await?;
    let alice = crate::test_support::Account::new().await?;
    let bob = crate::test_support::Account::new().await?;
    let has_no_code = crate::test_support::Account::new().await?;

    let safe_erc20_addr =
        alice.as_deployer_with_initial_owner().deploy().await?.contract_address;
    let safe_erc20_alice = SafeErc20::new(safe_erc20_addr, &alice.wallet);
    let bob_addr = bob.address();
    let has_no_code_addr = has_no_code.address();

    let requested_decrease = uint!(1_U256);

    let err = send!(safe_erc20_alice.safeDecreaseAllowance(
        has_no_code_addr,
        bob_addr,
        requested_decrease
    ))
    .expect_err("should not be able to invoke 'decreaseAllowance' on EOA");
    assert!(err.reverted_with(SafeErc20::SafeErc20FailedOperation {
        token: has_no_code_addr
    }));

    Ok(())
}

#[tokio::test]
async fn reverts_on_force_approve() -> eyre::Result<()> {
    let _test_context = crate::test_support::TestContext::new().await?;
    let alice = crate::test_support::Account::new().await?;
    let bob = crate::test_support::Account::new().await?;
    let has_no_code = crate::test_support::Account::new().await?;

    let safe_erc20_addr =
        alice.as_deployer_with_initial_owner().deploy().await?.contract_address;
    let safe_erc20_alice = SafeErc20::new(safe_erc20_addr, &alice.wallet);
    let bob_addr = bob.address();
    let has_no_code_addr = has_no_code.address();

    let err = send!(safe_erc20_alice.forceApprove(
        has_no_code_addr,
        bob_addr,
        U256::ZERO
    ))
    .expect_err("should not be able to invoke 'forceApprove' on EOA");
    assert!(err.reverted_with(SafeErc20::SafeErc20FailedOperation {
        token: has_no_code_addr
    }));

    Ok(())
}
