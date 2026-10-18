#![cfg(feature = "e2e")]

#[path = "../../devnet_test_support.rs"]
mod devnet_test_support;

use crate::devnet_test_support::{send, Account, Constructor, Revert};
use abi::Erc721;
use alloy::primitives::{Address, U256};
use eyre::Result;

mod abi;

fn constructor_for(receiver: Address) -> Constructor {
    Constructor::new(vec![
        format!("[{receiver}]"),
        "[1]".to_owned(),
        "0".to_owned(),
        "5000".to_owned(),
    ])
}

#[tokio::test]
async fn non_owner_cannot_mint() -> Result<()> {
    let alice = Account::new().await?;
    let bob = Account::new().await?;
    let contract_address = alice
        .as_deployer()
        .with_constructor(constructor_for(alice.address()))
        .deploy()
        .await?
        .contract_address;
    let contract = Erc721::new(contract_address, &bob.wallet);
    let token_id = U256::from(42);

    let error = send!(contract.mint(bob.address(), token_id))
        .expect_err("a non-owner must not mint NFTs");
    assert!(error.reverted_with(Erc721::OwnableUnauthorizedAccount {
        account: bob.address()
    }));
    assert_eq!(U256::ZERO, contract.balanceOf(bob.address()).call().await?);
    assert!(contract.ownerOf(token_id).call().await.is_err());

    Ok(())
}
