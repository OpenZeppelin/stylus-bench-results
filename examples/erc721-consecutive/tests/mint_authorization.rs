#![cfg(feature = "e2e")]

#[path = "../../devnet_test_support.rs"]
mod devnet_test_support;

use crate::devnet_test_support::{Account, Constructor, Revert, send};
use abi::Erc721;
use alloy::primitives::{Address, U256, aliases::U96, uint};
use eyre::Result;

mod abi;

const FIRST_CONSECUTIVE_ID: U96 = U96::ZERO;
const MAX_BATCH_SIZE: U96 = uint!(5000_U96);

fn ctr(receivers: &[Address], amounts: &[U96]) -> Constructor {
    let receivers = receivers
        .iter()
        .map(|receiver| format!("{receiver}"))
        .collect::<Vec<_>>()
        .join(",");
    let amounts = amounts
        .iter()
        .map(|amount| format!("{amount}"))
        .collect::<Vec<_>>()
        .join(",");
    Constructor::new(vec![
        format!("[{receivers}]"),
        format!("[{amounts}]"),
        FIRST_CONSECUTIVE_ID.to_string(),
        MAX_BATCH_SIZE.to_string(),
    ])
}

#[tokio::test]
async fn non_owner_cannot_mint() -> Result<()> {
    let alice = Account::new().await?;
    let bob = Account::new().await?;
    let receivers = vec![alice.address()];
    let amounts = vec![uint!(1_U96)];
    let contract_address = alice
        .as_deployer()
        .with_signer(&alice)
        .with_constructor(ctr(&receivers, &amounts))
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
