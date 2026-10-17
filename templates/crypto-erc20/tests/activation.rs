#![cfg(feature = "devnet")]

use alloy::{primitives::U256, sol};
use eyre::Result;
use stylus_test_support::{constructor, Account};

sol! {
    #[sol(rpc)]
    contract CryptoErc20 {
        function name() external view returns (string);
        function symbol() external view returns (string);
        function poseidonHash(uint256[2] inputs) external returns (uint256);
    }
}

#[tokio::test]
async fn deploys_activates_and_runs_poseidon() -> Result<()> {
    let account = Account::new().await?;
    let address = account
        .as_deployer()
        .with_build_feature("devnet")
        .with_constructor(constructor!(
            "Template Token".to_string(),
            "TMP".to_string()
        ))
        .deploy()
        .await?
        .contract_address;
    let token = CryptoErc20::new(address, &account.wallet);

    assert_eq!(token.name().call().await?, "Template Token");
    assert_eq!(token.symbol().call().await?, "TMP");
    let hash =
        token.poseidonHash([U256::from(1), U256::from(2)]).call().await?;
    assert_ne!(hash, U256::ZERO);
    Ok(())
}
