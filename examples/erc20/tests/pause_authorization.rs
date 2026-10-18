#![cfg(feature = "e2e")]

#[path = "../../devnet_test_support.rs"]
mod devnet_test_support;

use crate::devnet_test_support::{constructor, send, watch, Account};
use abi::Erc20;
use alloy::primitives::U256;
use eyre::Result;

mod abi;

fn deploy_args() -> devnet_test_support::Constructor {
    constructor!("Test Token".to_owned(), "TTK".to_owned(), U256::from(100))
}

#[tokio::test]
async fn non_admin_cannot_pause() -> Result<()> {
    let admin = Account::new().await?;
    let bob = Account::new().await?;
    let contract_address = admin
        .as_deployer()
        .with_owner(&admin)
        .with_constructor(deploy_args())
        .deploy()
        .await?
        .contract_address;
    let contract = Erc20::new(contract_address, &bob.wallet);

    send!(contract.pause()).expect_err("a non-admin must not pause the token");
    assert!(!contract.paused().call().await?);

    Ok(())
}

#[tokio::test]
async fn non_admin_cannot_unpause() -> Result<()> {
    let admin = Account::new().await?;
    let bob = Account::new().await?;
    let contract_address = admin
        .as_deployer()
        .with_owner(&admin)
        .with_constructor(deploy_args())
        .deploy()
        .await?
        .contract_address;
    let admin_contract = Erc20::new(contract_address, &admin.wallet);
    let bob_contract = Erc20::new(contract_address, &bob.wallet);

    watch!(admin_contract.pause())?;
    assert!(admin_contract.paused().call().await?);

    send!(bob_contract.unpause())
        .expect_err("a non-admin must not unpause the token");
    assert!(admin_contract.paused().call().await?);

    Ok(())
}
