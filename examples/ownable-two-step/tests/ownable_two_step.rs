#![cfg(feature = "e2e")]
#[path = "../../devnet_test_support.rs"]
mod devnet_test_support;

use crate::devnet_test_support::{
    constructor, receipt, send, Account, Constructor,
    ContractInitializationError, EventExt, Revert,
};
use abi::{
    Ownable2Step,
    Ownable2Step::{OwnershipTransferStarted, OwnershipTransferred},
};
use alloy::primitives::Address;
use eyre::Result;

mod abi;

fn ctr(owner: Address) -> Constructor {
    constructor!(owner)
}

// ============================================================================
// Integration Tests: Ownable2Step
// ============================================================================

#[tokio::test]
async fn constructs() -> Result<()> {
    let alice = Account::new().await?;

    let alice_addr = alice.address();
    let receipt =
        alice.as_deployer().with_constructor(ctr(alice_addr)).deploy().await?;
    let contract = Ownable2Step::new(receipt.contract_address, &alice.wallet);

    assert!(receipt.emits(OwnershipTransferred {
        previousOwner: Address::ZERO,
        newOwner: alice_addr,
    }));

    let owner = contract.owner().call().await?;
    assert_eq!(owner, alice_addr);

    let pending_owner = contract.pendingOwner().call().await?;
    assert_eq!(pending_owner, Address::ZERO);

    Ok(())
}

#[tokio::test]
async fn construct_reverts_when_owner_is_zero_address() -> Result<()> {
    let alice = Account::new().await?;

    let err = alice
        .as_deployer()
        .with_constructor(ctr(Address::ZERO))
        .deploy()
        .await
        .expect_err("should not deploy due to `OwnableInvalidOwner`");

    // StylusDeployer reports this constructor failure without exposing its
    // revert data.

    assert!(err.downcast_ref::<ContractInitializationError>().is_some());

    Ok(())
}

#[tokio::test]
async fn transfer_ownership_initiates_transfer() -> Result<()> {
    let alice = Account::new().await?;
    let bob = Account::new().await?;

    let alice_addr = alice.address();
    let bob_addr = bob.address();

    let contract_addr = alice
        .as_deployer()
        .with_constructor(ctr(alice_addr))
        .deploy()
        .await?
        .contract_address;
    let contract = Ownable2Step::new(contract_addr, &alice.wallet);

    let receipt = receipt!(contract.transferOwnership(bob_addr))?;
    assert!(receipt.emits(OwnershipTransferStarted {
        previousOwner: alice_addr,
        newOwner: bob_addr,
    }));

    // Current owner is still Alice
    let owner = contract.owner().call().await?;
    assert_eq!(owner, alice_addr);

    // Pending owner is Bob
    let pending_owner = contract.pendingOwner().call().await?;
    assert_eq!(pending_owner, bob_addr);

    Ok(())
}

#[tokio::test]
async fn transfer_ownership_reverts_when_not_owner() -> Result<()> {
    let alice = Account::new().await?;
    let bob = Account::new().await?;

    let alice_addr = alice.address();
    let bob_addr = bob.address();

    let contract_addr = alice
        .as_deployer()
        .with_constructor(ctr(bob_addr))
        .deploy()
        .await?
        .contract_address;
    let contract = Ownable2Step::new(contract_addr, &alice.wallet);

    let err = send!(contract.transferOwnership(bob_addr))
        .expect_err("should not transfer when not owned");
    err.reverted_with(Ownable2Step::OwnableUnauthorizedAccount {
        account: alice_addr,
    });

    Ok(())
}

#[tokio::test]
async fn accept_ownership() -> Result<()> {
    let alice = Account::new().await?;
    let bob = Account::new().await?;

    let alice_addr = alice.address();
    let bob_addr = bob.address();

    let contract_addr = alice
        .as_deployer()
        .with_constructor(ctr(alice_addr))
        .deploy()
        .await?
        .contract_address;
    let contract = Ownable2Step::new(contract_addr, &alice.wallet);
    receipt!(contract.transferOwnership(bob_addr))?;

    // Connect as Bob and accept ownership
    let contract = Ownable2Step::new(contract_addr, &bob.wallet);
    let receipt = receipt!(contract.acceptOwnership())?;
    assert!(receipt.emits(OwnershipTransferred {
        previousOwner: alice_addr,
        newOwner: bob_addr,
    }));

    let owner = contract.owner().call().await?;
    assert_eq!(owner, bob_addr);

    let pending_owner = contract.pendingOwner().call().await?;
    assert_eq!(pending_owner, Address::ZERO);

    Ok(())
}

#[tokio::test]
async fn transfer_ownership_cancel_transfer() -> Result<()> {
    let alice = Account::new().await?;
    let bob = Account::new().await?;

    let alice_addr = alice.address();
    let bob_addr = bob.address();

    let contract_addr = alice
        .as_deployer()
        .with_constructor(ctr(alice_addr))
        .deploy()
        .await?
        .contract_address;
    let contract = Ownable2Step::new(contract_addr, &alice.wallet);
    receipt!(contract.transferOwnership(bob_addr))?;

    let receipt = receipt!(contract.transferOwnership(Address::ZERO))?;
    assert!(receipt.emits(OwnershipTransferStarted {
        previousOwner: alice_addr,
        newOwner: Address::ZERO,
    }));

    let pending_owner = contract.pendingOwner().call().await?;
    assert_eq!(pending_owner, Address::ZERO);

    Ok(())
}

#[tokio::test]
async fn overwrite_previous_transfer_ownership() -> Result<()> {
    let alice = Account::new().await?;
    let bob = Account::new().await?;
    let charlie = Account::new().await?;

    let alice_addr = alice.address();
    let bob_addr = bob.address();
    let charlie_addr = charlie.address();

    let contract_addr = alice
        .as_deployer()
        .with_constructor(ctr(alice_addr))
        .deploy()
        .await?
        .contract_address;

    let contract = Ownable2Step::new(contract_addr, &alice.wallet);

    let receipt = receipt!(contract.transferOwnership(bob_addr))?;
    assert!(receipt.emits(OwnershipTransferStarted {
        previousOwner: alice_addr,
        newOwner: bob_addr,
    }));

    let receipt = receipt!(contract.transferOwnership(charlie_addr))?;
    assert!(receipt.emits(OwnershipTransferStarted {
        previousOwner: alice_addr,
        newOwner: charlie_addr,
    }));

    let pending_owner = contract.pendingOwner().call().await?;
    assert_eq!(pending_owner, charlie_addr);

    // Connect as Bob and try to accept ownership
    let contract = Ownable2Step::new(contract_addr, &bob.wallet);
    let err = send!(contract.acceptOwnership())
        .expect_err("should not accept when not pending owner");

    err.reverted_with(Ownable2Step::OwnableUnauthorizedAccount {
        account: bob_addr,
    });

    // Connect as Charlie and accept ownership
    let contract = Ownable2Step::new(contract_addr, &charlie.wallet);
    let receipt = receipt!(contract.acceptOwnership())?;
    assert!(receipt.emits(OwnershipTransferred {
        previousOwner: alice_addr,
        newOwner: charlie_addr,
    }));

    let pending_owner = contract.pendingOwner().call().await?;
    assert_eq!(pending_owner, Address::ZERO);

    let owner = contract.owner().call().await?;
    assert_eq!(owner, charlie_addr);

    Ok(())
}

#[tokio::test]
async fn accept_ownership_reverts_when_not_pending_owner() -> Result<()> {
    let alice = Account::new().await?;
    let bob = Account::new().await?;
    let charlie = Account::new().await?;

    let alice_addr = alice.address();
    let bob_addr = bob.address();
    let charlie_addr = charlie.address();
    let contract_addr = alice
        .as_deployer()
        .with_constructor(ctr(alice_addr))
        .deploy()
        .await?
        .contract_address;
    let contract = Ownable2Step::new(contract_addr, &alice.wallet);
    receipt!(contract.transferOwnership(bob_addr))?;

    // Connect as Charlie and attempt to accept ownership
    let contract = Ownable2Step::new(contract_addr, &charlie.wallet);
    let err = send!(contract.acceptOwnership())
        .expect_err("should not accept when not pending owner");
    err.reverted_with(Ownable2Step::OwnableUnauthorizedAccount {
        account: charlie_addr,
    });

    Ok(())
}

#[tokio::test]
async fn renounce_ownership() -> Result<()> {
    let alice = Account::new().await?;

    let alice_addr = alice.address();
    let contract_addr = alice
        .as_deployer()
        .with_constructor(ctr(alice_addr))
        .deploy()
        .await?
        .contract_address;
    let contract = Ownable2Step::new(contract_addr, &alice.wallet);

    let receipt = receipt!(contract.renounceOwnership())?;
    assert!(receipt.emits(OwnershipTransferred {
        previousOwner: alice_addr,
        newOwner: Address::ZERO,
    }));

    let owner = contract.owner().call().await?;
    assert_eq!(owner, Address::ZERO);

    // Pending owner is set to zero
    let pending_owner = contract.pendingOwner().call().await?;
    assert_eq!(pending_owner, Address::ZERO);

    Ok(())
}

#[tokio::test]
async fn renounce_ownership_reverts_when_not_owner() -> Result<()> {
    let alice = Account::new().await?;
    let bob = Account::new().await?;

    let alice_addr = alice.address();
    let bob_addr = bob.address();

    let contract_addr = alice
        .as_deployer()
        .with_constructor(ctr(alice_addr))
        .deploy()
        .await?
        .contract_address;
    let contract = Ownable2Step::new(contract_addr, &bob.wallet);

    let err = send!(contract.renounceOwnership())
        .expect_err("should prevent non-owner from renouncing");
    err.reverted_with(Ownable2Step::OwnableUnauthorizedAccount {
        account: bob_addr,
    });

    let owner = contract.owner().call().await?;
    assert_eq!(owner, alice_addr);

    Ok(())
}
