#![cfg(feature = "e2e")]

#[path = "../../devnet_test_support.rs"]
mod devnet_test_support;

use crate::devnet_test_support::{constructor, send, Account};
use abi::Erc20;
use alloy::primitives::U256;
use eyre::Result;

mod abi;

fn deploy_args() -> devnet_test_support::Constructor {
    constructor!("Test Token".to_owned(), "TTK".to_owned(), U256::from(100))
}

#[tokio::test]
async fn non_deployer_cannot_mint() -> Result<()> {
    let alice = Account::new().await?;
    let bob = Account::new().await?;
    let contract_address = alice
        .as_deployer()
        .with_constructor(deploy_args())
        .deploy()
        .await?
        .contract_address;
    let contract = Erc20::new(contract_address, &bob.wallet);

    send!(contract.mint(bob.address(), U256::from(1)))
        .expect_err("a non-minter must not mint tokens");
    assert_eq!(U256::ZERO, contract.totalSupply().call().await?);
    assert_eq!(U256::ZERO, contract.balanceOf(bob.address()).call().await?);

    Ok(())
}
