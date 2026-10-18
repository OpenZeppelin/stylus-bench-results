#![cfg(feature = "e2e")]
#[path = "../../devnet_test_support.rs"]
mod devnet_test_support;

use crate::devnet_test_support::{constructor, receipt, send, Account, Revert};
use abi::Erc1967BeaconRollbackExample;
use alloy::primitives::{Address, Bytes};
use eyre::Result;

mod abi;

#[tokio::test]
async fn invalid_beacon_implementation_reverts_and_preserves_beacon_slot(
) -> Result<()> {
    let alice = Account::new().await?;
    let no_code_address = Account::new().await?.address();

    // This fixture has code and can serve as a valid implementation target.
    let implementation = alice
        .as_deployer()
        .with_constructor(constructor!(no_code_address))
        .deploy()
        .await?
        .contract_address;

    // The valid beacon points at code; the second fixture returns an EOA.
    let valid_beacon_address = alice
        .as_deployer()
        .with_constructor(constructor!(implementation))
        .deploy()
        .await?
        .contract_address;
    let invalid_beacon_address = alice
        .as_deployer()
        .with_constructor(constructor!(no_code_address))
        .deploy()
        .await?
        .contract_address;
    let target_address = alice
        .as_deployer()
        .with_constructor(constructor!(implementation))
        .deploy()
        .await?
        .contract_address;

    let valid_beacon =
        Erc1967BeaconRollbackExample::new(valid_beacon_address, &alice.wallet);
    let target =
        Erc1967BeaconRollbackExample::new(target_address, &alice.wallet);
    assert_eq!(target.implementation().call().await?, implementation);
    assert_eq!(target.getBeacon().call().await?, Address::ZERO);
    assert_eq!(valid_beacon.implementation().call().await?, implementation);

    receipt!(target.upgradeBeaconToAndCall(valid_beacon_address, Bytes::new()))?;
    assert_eq!(target.getBeacon().call().await?, valid_beacon_address);

    let error = send!(
        target.upgradeBeaconToAndCall(invalid_beacon_address, Bytes::new())
    )
    .expect_err("beacon with an implementation EOA must be rejected");
    assert!(error.reverted_with(
        Erc1967BeaconRollbackExample::ERC1967InvalidImplementation {
            implementation: no_code_address,
        }
    ));

    // TestVM does not model this rollback, so verify the EVM invariant here.
    assert_eq!(target.getBeacon().call().await?, valid_beacon_address);

    Ok(())
}
