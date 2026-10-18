#![cfg(feature = "e2e")]
#[path = "../../devnet_test_support.rs"]
mod devnet_test_support;

use crate::devnet_test_support::{
    receipt, send, watch, Account, Constructor, ContractInitializationError,
    EventExt, Revert,
};
use alloy::primitives::{Address, U256};
use alloy_primitives::{aliases::U96, uint};

use crate::abi::Erc721;

mod abi;

const FIRST_CONSECUTIVE_ID: U96 = U96::ZERO;
const MAX_BATCH_SIZE: U96 = uint!(5000_U96);

fn random_token_id() -> U256 {
    let num: u32 = rand::random();
    U256::from(num)
}

fn ctr(receivers: &[Address], amounts: &[U96]) -> Constructor {
    let receivers =
        receivers.iter().map(|r| format!("{r}")).collect::<Vec<_>>().join(",");
    let amounts =
        amounts.iter().map(|r| format!("{r}")).collect::<Vec<_>>().join(",");

    Constructor::new(vec![
        format!("[{receivers}]"),
        format!("[{amounts}]"),
        FIRST_CONSECUTIVE_ID.to_string(),
        MAX_BATCH_SIZE.to_string(),
    ])
}

#[tokio::test]
async fn constructs() -> eyre::Result<()> {
    let alice = Account::new().await?;

    let alice_addr = alice.address();
    let receivers = vec![alice_addr];
    let amounts = vec![uint!(10_U96)];
    let receipt = alice
        .as_deployer()
        .with_owner(&alice)
        .with_constructor(ctr(&receivers, &amounts))
        .deploy()
        .await?;
    let contract = Erc721::new(receipt.contract_address, &alice.wallet);

    let balance = contract.balanceOf(alice_addr).call().await?;
    assert_eq!(balance, uint!(10_U256));
    Ok(())
}

#[tokio::test]
async fn sdk_constructor_route_mints_multiple_batches() -> eyre::Result<()> {
    let alice = Account::new().await?;
    let bob = Account::new().await?;

    let receivers = vec![alice.address(), bob.address()];
    let amounts = vec![uint!(2_U96), uint!(3_U96)];
    let receipt = alice
        .as_deployer()
        .with_owner(&alice)
        .with_constructor(ctr(&receivers, &amounts))
        .deploy()
        .await?;
    let contract = Erc721::new(receipt.contract_address, &alice.wallet);

    assert_eq!(
        contract.balanceOf(alice.address()).call().await?,
        uint!(2_U256)
    );
    assert_eq!(contract.balanceOf(bob.address()).call().await?, uint!(3_U256));
    assert_eq!(contract.ownerOf(U256::ZERO).call().await?, alice.address());
    assert_eq!(contract.ownerOf(uint!(1_U256)).call().await?, alice.address());
    assert_eq!(contract.ownerOf(uint!(2_U256)).call().await?, bob.address());
    assert_eq!(contract.ownerOf(uint!(4_U256)).call().await?, bob.address());

    Ok(())
}

#[tokio::test]
async fn mints() -> eyre::Result<()> {
    let alice = Account::new().await?;

    let batch_size = uint!(10_U96);
    let receivers = vec![alice.address()];
    let amounts = vec![batch_size];
    let receipt = alice
        .as_deployer()
        .with_owner(&alice)
        .with_constructor(ctr(&receivers, &amounts))
        .deploy()
        .await?;
    let contract = Erc721::new(receipt.contract_address, &alice.wallet);

    assert!(receipt.emits(Erc721::ConsecutiveTransfer {
        fromTokenId: U256::from(FIRST_CONSECUTIVE_ID),
        toTokenId: uint!(9_U256),
        fromAddress: Address::ZERO,
        toAddress: alice.address(),
    }));

    let balance1 = contract.balanceOf(alice.address()).call().await?;
    assert_eq!(balance1, U256::from(batch_size));

    let token_id = random_token_id();
    watch!(contract.mint(alice.address(), token_id))?;

    let balance2 = contract.balanceOf(alice.address()).call().await?;

    assert_eq!(balance2, balance1 + uint!(1_U256));
    Ok(())
}

#[tokio::test]
async fn error_when_to_is_zero() -> eyre::Result<()> {
    let alice = Account::new().await?;

    let receivers = vec![Address::ZERO];
    let amounts = vec![uint!(10_U96)];
    let err = alice
        .as_deployer()
        .with_owner(&alice)
        .with_constructor(ctr(&receivers, &amounts))
        .deploy()
        .await
        .expect_err("should not mint consecutive");

    // StylusDeployer reports this constructor failure without exposing its
    // revert data.

    assert!(err.downcast_ref::<ContractInitializationError>().is_some());

    Ok(())
}

#[tokio::test]
async fn error_when_exceed_batch_size() -> eyre::Result<()> {
    let alice = Account::new().await?;

    let receivers = vec![alice.address()];
    let amounts = vec![MAX_BATCH_SIZE + uint!(1_U96)];
    let err = alice
        .as_deployer()
        .with_owner(&alice)
        .with_constructor(ctr(&receivers, &amounts))
        .deploy()
        .await
        .expect_err("should not mint consecutive");

    // StylusDeployer reports this constructor failure without exposing its
    // revert data.

    assert!(err.downcast_ref::<ContractInitializationError>().is_some());

    Ok(())
}

#[tokio::test]
async fn transfers_from() -> eyre::Result<()> {
    let alice = Account::new().await?;
    let bob = Account::new().await?;

    let receivers = vec![alice.address(), bob.address()];
    let amounts = vec![uint!(1000_U96), uint!(1000_U96)];
    // Deploy and mint batches of 1000 tokens to Alice and Bob.
    let receipt = alice
        .as_deployer()
        .with_owner(&alice)
        .with_constructor(ctr(&receivers, &amounts))
        .deploy()
        .await?;
    let contract = Erc721::new(receipt.contract_address, &alice.wallet);

    let first_consecutive_token_id = U256::from(FIRST_CONSECUTIVE_ID);

    // Transfer first consecutive token from Alice to Bob.
    watch!(contract.transferFrom(
        alice.address(),
        bob.address(),
        first_consecutive_token_id
    ))?;

    let owner_of = contract.ownerOf(first_consecutive_token_id).call().await?;
    assert_eq!(owner_of, bob.address());

    // Check that balances changed.
    let alice_balance = contract.balanceOf(alice.address()).call().await?;
    assert_eq!(alice_balance, uint!(1000_U256) - uint!(1_U256));
    let bob_balance = contract.balanceOf(bob.address()).call().await?;
    assert_eq!(bob_balance, uint!(1000_U256) + uint!(1_U256));

    // Test non-consecutive mint.
    let token_id = random_token_id();
    watch!(contract.mint(alice.address(), token_id))?;
    let alice_balance = contract.balanceOf(alice.address()).call().await?;
    assert_eq!(alice_balance, uint!(1000_U256));

    // Test transfer of the token that wasn't minted consecutive.
    watch!(contract.transferFrom(alice.address(), bob.address(), token_id))?;
    let alice_balance = contract.balanceOf(alice.address()).call().await?;
    assert_eq!(alice_balance, uint!(1000_U256) - uint!(1_U256));
    Ok(())
}

#[tokio::test]
async fn burns() -> eyre::Result<()> {
    let alice = Account::new().await?;

    let receivers = vec![alice.address()];
    let amounts = vec![uint!(1000_U96)];
    // Mint batch of 1000 tokens to Alice.
    let receipt = alice
        .as_deployer()
        .with_owner(&alice)
        .with_constructor(ctr(&receivers, &amounts))
        .deploy()
        .await?;
    let contract = Erc721::new(receipt.contract_address, &alice.wallet);

    let first_consecutive_token_id = U256::from(FIRST_CONSECUTIVE_ID);

    // Check consecutive token burn.
    let receipt = receipt!(contract.burn(first_consecutive_token_id))?;

    assert!(receipt.emits(Erc721::Transfer {
        from: alice.address(),
        to: Address::ZERO,
        tokenId: first_consecutive_token_id,
    }));

    let alice_balance = contract.balanceOf(alice.address()).call().await?;
    assert_eq!(alice_balance, uint!(1000_U256) - uint!(1_U256));

    let err = contract
        .ownerOf(first_consecutive_token_id)
        .call()
        .await
        .expect_err("should return `ERC721NonexistentToken`");

    assert!(err.reverted_with(Erc721::ERC721NonexistentToken {
        tokenId: first_consecutive_token_id
    }));

    // Check non-consecutive token burn.
    let non_consecutive_token_id = random_token_id();
    watch!(contract.mint(alice.address(), non_consecutive_token_id))?;
    let owner_of = contract.ownerOf(non_consecutive_token_id).call().await?;
    assert_eq!(owner_of, alice.address());
    let alice_balance = contract.balanceOf(alice.address()).call().await?;
    assert_eq!(alice_balance, uint!(1000_U256));

    let receipt = receipt!(contract.burn(non_consecutive_token_id))?;

    assert!(receipt.emits(Erc721::Transfer {
        from: alice.address(),
        to: Address::ZERO,
        tokenId: non_consecutive_token_id,
    }));

    let err = contract
        .ownerOf(non_consecutive_token_id)
        .call()
        .await
        .expect_err("should return `ERC721NonexistentToken`");

    assert!(err.reverted_with(Erc721::ERC721NonexistentToken {
        tokenId: non_consecutive_token_id
    }));
    Ok(())
}

#[tokio::test]
async fn unauthorized_burn_reverts_and_preserves_consecutive_token(
) -> eyre::Result<()> {
    let alice = Account::new().await?;
    let bob = Account::new().await?;

    let receivers = vec![alice.address()];
    let amounts = vec![uint!(1_U96)];
    let contract_addr = alice
        .as_deployer()
        .with_owner(&alice)
        .with_constructor(ctr(&receivers, &amounts))
        .deploy()
        .await?
        .contract_address;
    let token_id = U256::from(FIRST_CONSECUTIVE_ID);
    let contract_alice = Erc721::new(contract_addr, &alice.wallet);
    let contract_bob = Erc721::new(contract_addr, &bob.wallet);

    let err = send!(contract_bob.burn(token_id))
        .expect_err("an unapproved account must not burn Alice's token");
    assert!(err.reverted_with(Erc721::ERC721InsufficientApproval {
        operator: bob.address(),
        tokenId: token_id,
    }));
    assert_eq!(contract_alice.ownerOf(token_id).call().await?, alice.address());

    Ok(())
}

// No need to test for `IErc165` impl, as this is already tested in
// ../../examples/erc721 e2e tests
