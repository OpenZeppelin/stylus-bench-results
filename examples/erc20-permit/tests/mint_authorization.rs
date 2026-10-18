#![cfg(feature = "e2e")]

#[path = "../../devnet_test_support.rs"]
mod devnet_test_support;

use crate::devnet_test_support::{send, Account};
use abi::Erc20Permit;
use alloy::primitives::U256;
use eyre::Result;

mod abi;

#[tokio::test]
async fn non_deployer_cannot_mint() -> Result<()> {
    let alice = Account::new().await?;
    let bob = Account::new().await?;
    let contract_address =
        alice.as_deployer().with_owner(&alice).deploy().await?.contract_address;
    let contract = Erc20Permit::new(contract_address, &bob.wallet);

    send!(contract.mint(bob.address(), U256::from(1)))
        .expect_err("a non-minter must not mint tokens");
    assert_eq!(U256::ZERO, contract.totalSupply().call().await?);
    assert_eq!(U256::ZERO, contract.balanceOf(bob.address()).call().await?);

    Ok(())
}
