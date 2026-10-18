#![cfg(feature = "e2e")]
#[path = "../../devnet_test_support.rs"]
mod devnet_test_support;

use crate::devnet_test_support::{
    Account, Constructor, EventExt, Revert, constructor, receipt, send, watch,
};
use abi::Erc1155;
use alloy::primitives::U256;

mod abi;

const URI: &str = "https://github.com/OpenZeppelin/rust-contracts-stylus";
const BASE_URI: &str = "https://github.com";

fn ctr(uri: &str) -> Constructor {
    constructor!(uri.to_string())
}

// ============================================================================
// Integration Tests: ERC-1155 URI Storage Extension
// ============================================================================

#[tokio::test]
async fn uri_returns_metadata_uri_when_token_uri_is_not_set() -> eyre::Result<()>
{
    let alice = Account::new().await?;

    let contract_addr = alice
        .as_deployer()
        .with_signer(&alice)
        .with_constructor(ctr(URI))
        .deploy()
        .await?
        .contract_address;

    let contract = Erc1155::new(contract_addr, &alice.wallet);

    let token_id = U256::from(1);

    let uri = contract.uri(token_id).call().await?;

    assert_eq!(URI, uri);
    Ok(())
}

#[tokio::test]
async fn uri_returns_empty_string_when_no_uri_is_set() -> eyre::Result<()> {
    let alice = Account::new().await?;

    let contract_addr = alice
        .as_deployer()
        .with_signer(&alice)
        .with_constructor(ctr(""))
        .deploy()
        .await?
        .contract_address;

    let contract = Erc1155::new(contract_addr, &alice.wallet);

    let token_id = U256::from(1);

    let uri = contract.uri(token_id).call().await?;

    assert_eq!("", uri);

    Ok(())
}

#[tokio::test]
async fn uri_returns_concatenated_base_uri_and_token_uri() -> eyre::Result<()> {
    let alice = Account::new().await?;

    let contract_addr = alice
        .as_deployer()
        .with_signer(&alice)
        .with_constructor(ctr(""))
        .deploy()
        .await?
        .contract_address;

    let contract = Erc1155::new(contract_addr, &alice.wallet);

    let token_id = U256::from(1);
    let token_uri = "/some/token/uri";
    let expected_uri = BASE_URI.to_owned() + token_uri;

    watch!(contract.setBaseURI(BASE_URI.to_owned()))?;

    let receipt =
        receipt!(contract.setTokenURI(token_id, token_uri.to_owned()))?;

    assert!(
        receipt
            .emits(Erc1155::URI { value: expected_uri.clone(), id: token_id })
    );

    let uri = contract.uri(token_id).call().await?;

    assert_eq!(expected_uri, uri);

    Ok(())
}

#[tokio::test]
async fn uri_returns_token_uri_when_base_uri_is_empty() -> eyre::Result<()> {
    let alice = Account::new().await?;

    let contract_addr = alice
        .as_deployer()
        .with_signer(&alice)
        .with_constructor(ctr(""))
        .deploy()
        .await?
        .contract_address;

    let contract = Erc1155::new(contract_addr, &alice.wallet);

    let token_id = U256::from(1);
    let token_uri = "https://random.uri/some/token/uri";

    let receipt =
        receipt!(contract.setTokenURI(token_id, token_uri.to_owned()))?;

    assert!(
        receipt
            .emits(Erc1155::URI { value: token_uri.to_owned(), id: token_id })
    );

    let uri = contract.uri(token_id).call().await?;

    assert_eq!(token_uri, uri);

    Ok(())
}

#[tokio::test]
async fn uri_ignores_metadata_uri_when_token_uri_is_set() -> eyre::Result<()> {
    let alice = Account::new().await?;

    let contract_addr = alice
        .as_deployer()
        .with_signer(&alice)
        .with_constructor(ctr(URI))
        .deploy()
        .await?
        .contract_address;

    let contract = Erc1155::new(contract_addr, &alice.wallet);

    let token_id = U256::from(1);
    let token_uri = "/some/token/uri";
    let expected_uri = BASE_URI.to_owned() + token_uri;

    watch!(contract.setBaseURI(BASE_URI.to_owned()))?;

    let receipt =
        receipt!(contract.setTokenURI(token_id, token_uri.to_owned()))?;

    assert!(
        receipt
            .emits(Erc1155::URI { value: expected_uri.clone(), id: token_id })
    );

    let uri = contract.uri(token_id).call().await?;

    assert_eq!(expected_uri, uri);

    Ok(())
}

#[tokio::test]
async fn unauthorized_account_cannot_change_token_uris() -> eyre::Result<()> {
    let alice = Account::new().await?;
    let bob = Account::new().await?;

    let contract_addr = alice
        .as_deployer()
        .with_signer(&alice)
        .with_constructor(ctr(URI))
        .deploy()
        .await?
        .contract_address;
    let contract_alice = Erc1155::new(contract_addr, &alice.wallet);
    let contract_bob = Erc1155::new(contract_addr, &bob.wallet);
    let token_id = U256::from(1);

    let err = send!(
        contract_bob.setTokenURI(token_id, "attacker/metadata.json".to_owned())
    )
    .expect_err("an unrelated account must not replace a token URI");
    assert!(err.reverted_with(Erc1155::OwnableUnauthorizedAccount {
        account: bob.address(),
    }));
    let err = send!(contract_bob.initializeOwner(bob.address())).expect_err(
        "an initialized contract must reject ownership reinitialization",
    );
    assert!(err.reverted_with(Erc1155::OwnableUnauthorizedAccount {
        account: bob.address(),
    }));

    let err =
        send!(contract_bob.setBaseURI("https://attacker.invalid/".to_owned()))
            .expect_err("an unrelated account must not replace the base URI");
    assert!(err.reverted_with(Erc1155::OwnableUnauthorizedAccount {
        account: bob.address(),
    }));
    assert_eq!(contract_alice.uri(token_id).call().await?, URI);

    Ok(())
}
