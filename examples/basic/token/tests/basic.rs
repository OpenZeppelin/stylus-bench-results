#![cfg(feature = "devnet")]

use alloy::{primitives::U256, sol};
use eyre::Result;
use stylus_test_support::{constructor, Account};

sol! {
    #[sol(rpc)]
    contract BasicToken {
        function name() external view returns (string);
        function symbol() external view returns (string);
        function totalSupply() external view returns (uint256);
    }
}

#[tokio::test]
async fn deploys_and_activates() -> Result<()> {
    let account = Account::new().await?;
    let address = account
        .as_deployer()
        .with_build_feature("devnet")
        .with_constructor(constructor!(
            "Example Token".to_string(),
            "EXT".to_string()
        ))
        .deploy()
        .await?
        .contract_address;
    let token = BasicToken::new(address, &account.wallet);

    assert_eq!(token.name().call().await?, "Example Token");
    assert_eq!(token.symbol().call().await?, "EXT");
    assert_eq!(token.totalSupply().call().await?, U256::ZERO);
    Ok(())
}
