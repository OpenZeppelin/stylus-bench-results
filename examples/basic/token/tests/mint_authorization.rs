#![cfg(feature = "e2e")]

#[path = "../../../devnet_test_support.rs"]
mod devnet_test_support;

use crate::devnet_test_support::{constructor, send, Account};
use alloy::primitives::U256;
use alloy::sol;
use eyre::Result;

sol!(
    #[sol(rpc)]
    contract BasicToken {
        function mint(address account, uint256 value) external;
        function totalSupply() external view returns (uint256);
        function balanceOf(address account) external view returns (uint256);
    }
);

#[tokio::test]
async fn non_deployer_cannot_mint() -> Result<()> {
    let alice = Account::new().await?;
    let bob = Account::new().await?;
    let contract_address = alice
        .as_deployer()
        .with_signer(&alice)
        .with_constructor(constructor!(
            "Basic Token".to_owned(),
            "BASIC".to_owned()
        ))
        .deploy()
        .await?
        .contract_address;

    let contract = BasicToken::new(contract_address, &bob.wallet);
    let amount = U256::from(1);
    send!(contract.mint(alice.address(), amount))
        .expect_err("a non-deployer must not mint tokens");

    assert_eq!(U256::ZERO, contract.totalSupply().call().await?);
    assert_eq!(U256::ZERO, contract.balanceOf(alice.address()).call().await?);

    Ok(())
}
