#![cfg(feature = "integration-tests")]

#[path = "../../../test-support/mod.rs"]
mod test_support;

use crate::test_support::{constructor, receipt, Account, EventExt};
use abi::BasicToken;
use alloy::primitives::{Address, U256};
use eyre::Result;

mod abi;

#[tokio::test]
async fn deploys_and_runs_basic_token() -> Result<()> {
    let _test_context = test_support::TestContext::new().await?;
    let alice = Account::new().await?;
    let bob = Account::new().await?;

    let deployed = alice
        .as_deployer()
        .with_constructor(constructor!(
            "Basic Token".to_owned(),
            "BTK".to_owned()
        ))
        .deploy()
        .await?;
    let token = BasicToken::new(deployed.contract_address, &alice.wallet);

    assert_eq!(token.name().call().await?, "Basic Token");
    assert_eq!(token.symbol().call().await?, "BTK");
    assert_eq!(token.decimals().call().await?, 18);
    assert_eq!(token.totalSupply().call().await?, U256::ZERO);

    let amount = U256::from(100);
    let receipt = receipt!(token.mint(alice.address(), amount))?;
    assert!(receipt.emits(BasicToken::Transfer {
        from: Address::ZERO,
        to: alice.address(),
        value: amount,
    }));

    assert_eq!(token.balanceOf(alice.address()).call().await?, amount);
    assert_eq!(token.totalSupply().call().await?, amount);

    let transfer_receipt = receipt!(token.transfer(bob.address(), amount))?;
    assert!(transfer_receipt.emits(BasicToken::Transfer {
        from: alice.address(),
        to: bob.address(),
        value: amount,
    }));
    assert_eq!(token.balanceOf(alice.address()).call().await?, U256::ZERO);
    assert_eq!(token.balanceOf(bob.address()).call().await?, amount);
    assert_eq!(token.totalSupply().call().await?, amount);

    Ok(())
}
