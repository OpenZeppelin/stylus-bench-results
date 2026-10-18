#![cfg(feature = "e2e")]

#[path = "../../devnet_test_support.rs"]
mod devnet_test_support;

use crate::devnet_test_support::{send, Account};
use abi::Erc1155;
use alloy::primitives::U256;
use eyre::Result;

mod abi;

#[tokio::test]
async fn non_deployer_cannot_mint() -> Result<()> {
    let alice = Account::new().await?;
    let bob = Account::new().await?;
    let contract_address = alice.as_deployer().deploy().await?.contract_address;
    let contract = Erc1155::new(contract_address, &bob.wallet);
    let id = U256::from(7);

    send!(contract.mint(bob.address(), id, U256::from(1), vec![].into()))
        .expect_err("a non-minter must not mint tokens");
    assert_eq!(U256::ZERO, contract.balanceOf(bob.address(), id).call().await?);

    Ok(())
}

#[tokio::test]
async fn non_deployer_cannot_mint_batch() -> Result<()> {
    let alice = Account::new().await?;
    let bob = Account::new().await?;
    let contract_address = alice.as_deployer().deploy().await?.contract_address;
    let contract = Erc1155::new(contract_address, &bob.wallet);
    let ids = vec![U256::from(7), U256::from(8)];

    send!(contract.mintBatch(
        bob.address(),
        ids.clone(),
        vec![U256::from(1), U256::from(2)],
        vec![].into()
    ))
    .expect_err("a non-minter must not batch mint tokens");
    for id in ids {
        assert_eq!(
            U256::ZERO,
            contract.balanceOf(bob.address(), id).call().await?
        );
    }

    Ok(())
}
