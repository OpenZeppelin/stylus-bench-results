#![cfg(feature = "integration-tests")]

#[path = "../../test-support/mod.rs"]
mod test_support;

use abi::MerkleProofs;
use alloy::primitives::B256;
use eyre::Result;

mod abi;

#[tokio::test]
async fn deploys_and_verifies_single_leaf_proof() -> Result<()> {
    let _test_context = test_support::TestContext::new().await?;
    let alice = test_support::Account::new().await?;

    let deployed = alice.as_deployer().deploy().await?;
    let verifier = MerkleProofs::new(deployed.contract_address, &alice.wallet);

    let leaf = B256::with_last_byte(1);
    assert!(verifier.verify(vec![], leaf, leaf).call().await?);
    assert!(!verifier.verify(vec![], B256::ZERO, leaf).call().await?);

    Ok(())
}
