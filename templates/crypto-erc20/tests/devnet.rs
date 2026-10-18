#![cfg(feature = "integration-tests")]

#[path = "../../../examples/test-support/mod.rs"]
mod test_support;

use alloy::{
    primitives::{Address, U256},
    sol,
};
use eyre::Result;
use test_support::{constructor, receipt, Account, Ext, TestContext};

sol! {
    #[sol(rpc)]
    contract CryptoErc20 {
        function name() external view returns (string);
        function symbol() external view returns (string);
        function decimals() external view returns (uint8);
        function totalSupply() external view returns (uint256);
        function balanceOf(address account) external view returns (uint256);
        function mint(address to, uint256 value) external;

        #[derive(Debug, PartialEq)]
        event Transfer(address indexed from, address indexed to, uint256 value);
    }
}

#[tokio::test]
async fn deploys_activates_and_runs_the_crypto_erc20_template() -> Result<()> {
    let _test_context = TestContext::new().await?;
    let alice = Account::new().await?;

    let deployed = alice
        .as_deployer()
        .with_constructor(constructor!(
            "Stylus ERC-20 Workshop".to_owned(),
            "MTK".to_owned()
        ))
        .deploy()
        .await?;
    let token = CryptoErc20::new(deployed.contract_address, &alice.wallet);

    assert_eq!(token.name().call().await?, "Stylus ERC-20 Workshop");
    assert_eq!(token.symbol().call().await?, "MTK");
    assert_eq!(token.decimals().call().await?, 18);
    assert_eq!(token.totalSupply().call().await?, U256::ZERO);

    let amount = U256::from(100);
    let receipt = receipt!(token.mint(alice.address(), amount))?;
    assert!(receipt.emits(CryptoErc20::Transfer {
        from: Address::ZERO,
        to: alice.address(),
        value: amount,
    }));
    assert_eq!(token.balanceOf(alice.address()).call().await?, amount);

    Ok(())
}
