#![cfg(feature = "devnet")]

use alloy::{primitives::B256, sol};
use eyre::Result;
use stylus_test_support::Account;

sol! {
    #[sol(rpc)]
    contract MerkleVerifier {
        function verify(bytes32[] proof, bytes32 root, bytes32 leaf) external view returns (bool);
    }
}

#[tokio::test]
async fn deploys_activates_and_verifies_a_single_leaf() -> Result<()> {
    let account = Account::new().await?;
    let address = account
        .as_deployer()
        .with_build_feature("devnet")
        .deploy()
        .await?
        .contract_address;
    let verifier = MerkleVerifier::new(address, &account.wallet);
    let leaf = B256::repeat_byte(0x42);

    assert!(verifier.verify(vec![], leaf, leaf).call().await?);
    Ok(())
}
