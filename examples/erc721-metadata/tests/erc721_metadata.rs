#![cfg(feature = "integration-tests")]
#![allow(clippy::unreadable_literal)]

#[path = "../../test-support/mod.rs"]
mod test_support;
use crate::test_support::{
    constructor, receipt, watch, Account, Constructor, EventExt, Revert,
};
use abi::Erc721;
use alloy::primitives::{aliases::B32, Address, U256};

mod abi;

const TOKEN_NAME: &str = "Test Token";
const TOKEN_SYMBOL: &str = "NFT";

fn random_token_id() -> U256 {
    let num: u32 = rand::random();
    U256::from(num)
}

fn ctr(base_uri: &str) -> Constructor {
    constructor!(
        TOKEN_NAME.to_string(),
        TOKEN_SYMBOL.to_string(),
        base_uri.to_string()
    )
}

// ============================================================================
// Integration Tests: ERC-721 Metadata Extension
// ============================================================================

#[tokio::test]
async fn constructs() -> eyre::Result<()> {
    let _test_context = crate::test_support::TestContext::new().await?;
    let alice = crate::test_support::Account::new().await?;

    let contract_addr = alice
        .as_deployer()
        .with_constructor(ctr(
            "https://github.com/OpenZeppelin/rust-contracts-stylus",
        ))
        .deploy()
        .await?
        .contract_address;
    let contract = Erc721::new(contract_addr, &alice.wallet);

    let Erc721::nameReturn { name } = contract.name().call().await?;
    let Erc721::symbolReturn { symbol } = contract.symbol().call().await?;

    assert_eq!(TOKEN_NAME.to_owned(), name);
    assert_eq!(TOKEN_SYMBOL.to_owned(), symbol);

    Ok(())
}

// ============================================================================
// Integration Tests: ERC-721 URI Storage Extension
// ============================================================================

#[tokio::test]
async fn error_when_checking_token_uri_for_nonexistent_token(
) -> eyre::Result<()> {
    let _test_context = crate::test_support::TestContext::new().await?;
    let alice = crate::test_support::Account::new().await?;

    let contract_addr = alice
        .as_deployer()
        .with_constructor(ctr(""))
        .deploy()
        .await?
        .contract_address;

    let contract = Erc721::new(contract_addr, &alice.wallet);

    let token_id = random_token_id();

    let err = contract
        .tokenURI(token_id)
        .call()
        .await
        .expect_err("should return ERC721NonexistentToken");

    assert!(
        err.reverted_with(Erc721::ERC721NonexistentToken { tokenId: token_id })
    );
    Ok(())
}

#[tokio::test]
async fn return_empty_token_uri_when_without_base_uri_and_token_uri(
) -> eyre::Result<()> {
    let _test_context = crate::test_support::TestContext::new().await?;
    let alice = crate::test_support::Account::new().await?;

    let contract_addr = alice
        .as_deployer()
        .with_constructor(ctr(""))
        .deploy()
        .await?
        .contract_address;

    let contract = Erc721::new(contract_addr, &alice.wallet);

    let token_id = random_token_id();

    watch!(contract.mint(alice.address(), token_id))?;

    let Erc721::tokenURIReturn { tokenURI } =
        contract.tokenURI(token_id).call().await?;

    assert_eq!("", tokenURI);

    Ok(())
}

#[tokio::test]
async fn return_token_uri_with_base_uri_and_without_token_uri(
) -> eyre::Result<()> {
    let _test_context = crate::test_support::TestContext::new().await?;
    let alice = crate::test_support::Account::new().await?;

    let base_uri = "https://github.com/OpenZeppelin/rust-contracts-stylus/";

    let contract_addr = alice
        .as_deployer()
        .with_constructor(ctr(base_uri))
        .deploy()
        .await?
        .contract_address;

    let contract = Erc721::new(contract_addr, &alice.wallet);

    let token_id = random_token_id();

    watch!(contract.mint(alice.address(), token_id))?;

    let Erc721::tokenURIReturn { tokenURI } =
        contract.tokenURI(token_id).call().await?;

    assert_eq!(base_uri.to_owned() + &token_id.to_string(), tokenURI);
    Ok(())
}

#[tokio::test]
async fn return_token_uri_with_base_uri_and_token_uri() -> eyre::Result<()> {
    let _test_context = crate::test_support::TestContext::new().await?;
    let alice = crate::test_support::Account::new().await?;

    let base_uri = "https://github.com/OpenZeppelin/rust-contracts-stylus/";

    let contract_addr = alice
        .as_deployer()
        .with_constructor(ctr(base_uri))
        .deploy()
        .await?
        .contract_address;

    let contract = Erc721::new(contract_addr, &alice.wallet);

    let token_id = random_token_id();

    watch!(contract.mint(alice.address(), token_id))?;

    let token_uri = String::from(
        "blob/main/contracts/src/token/erc721/extensions/uri_storage.rs",
    );

    let receipt = receipt!(contract.setTokenURI(token_id, token_uri.clone()))?;

    assert!(receipt.emits(Erc721::MetadataUpdate { tokenId: token_id }));

    let Erc721::tokenURIReturn { tokenURI } =
        contract.tokenURI(token_id).call().await?;

    assert_eq!(base_uri.to_owned() + &token_uri, tokenURI);

    Ok(())
}

#[tokio::test]
async fn set_token_uri_before_mint() -> eyre::Result<()> {
    let _test_context = crate::test_support::TestContext::new().await?;
    let alice = crate::test_support::Account::new().await?;

    let base_uri = "https://github.com/OpenZeppelin/rust-contracts-stylus/";

    let contract_addr = alice
        .as_deployer()
        .with_constructor(ctr(base_uri))
        .deploy()
        .await?
        .contract_address;

    let contract = Erc721::new(contract_addr, &alice.wallet);

    let token_id = random_token_id();

    let err = contract
        .ownerOf(token_id)
        .call()
        .await
        .expect_err("should return `ERC721NonexistentToken`");

    assert!(
        err.reverted_with(Erc721::ERC721NonexistentToken { tokenId: token_id })
    );

    let token_uri = String::from(
        "blob/main/contracts/src/token/erc721/extensions/uri_storage.rs",
    );

    let receipt = receipt!(contract.setTokenURI(token_id, token_uri.clone()))?;

    assert!(receipt.emits(Erc721::MetadataUpdate { tokenId: token_id }));

    watch!(contract.mint(alice.address(), token_id))?;

    let Erc721::tokenURIReturn { tokenURI } =
        contract.tokenURI(token_id).call().await?;

    assert_eq!(base_uri.to_owned() + &token_uri, tokenURI);

    Ok(())
}

#[tokio::test]
async fn return_token_uri_after_burn_and_remint() -> eyre::Result<()> {
    let _test_context = crate::test_support::TestContext::new().await?;
    let alice = crate::test_support::Account::new().await?;

    let base_uri = "https://github.com/OpenZeppelin/rust-contracts-stylus/";

    let alice_addr = alice.address();

    let contract_addr = alice
        .as_deployer()
        .with_constructor(ctr(base_uri))
        .deploy()
        .await?
        .contract_address;

    let contract = Erc721::new(contract_addr, &alice.wallet);

    let token_id = random_token_id();

    watch!(contract.mint(alice.address(), token_id))?;

    let receipt = receipt!(contract.burn(token_id))?;

    assert!(receipt.emits(Erc721::Transfer {
        from: alice_addr,
        to: Address::ZERO,
        tokenId: token_id,
    }));

    let err = contract
        .ownerOf(token_id)
        .call()
        .await
        .expect_err("should return `ERC721NonexistentToken`");

    assert!(
        err.reverted_with(Erc721::ERC721NonexistentToken { tokenId: token_id })
    );

    let receipt = receipt!(contract.mint(alice_addr, token_id))?;

    assert!(receipt.emits(Erc721::Transfer {
        from: Address::ZERO,
        to: alice_addr,
        tokenId: token_id
    }));

    let Erc721::ownerOfReturn { ownerOf: owner_of } =
        contract.ownerOf(token_id).call().await?;
    assert_eq!(alice_addr, owner_of);

    let Erc721::tokenURIReturn { tokenURI } =
        contract.tokenURI(token_id).call().await?;

    assert_eq!(base_uri.to_owned() + &token_id.to_string(), tokenURI);
    Ok(())
}

// ============================================================================
// Integration Tests: ERC-165 Support Interface
// ============================================================================

#[tokio::test]
async fn supports_interface() -> eyre::Result<()> {
    let _test_context = crate::test_support::TestContext::new().await?;
    let alice = crate::test_support::Account::new().await?;

    let contract_addr = alice
        .as_deployer()
        .with_constructor(ctr(
            "https://github.com/OpenZeppelin/rust-contracts-stylus",
        ))
        .deploy()
        .await?
        .contract_address;
    let contract = Erc721::new(contract_addr, &alice.wallet);

    let erc721_metadata_interface_id: B32 = 0x5b5e139f_u32.into();
    let supports_interface = contract
        .supportsInterface(erc721_metadata_interface_id)
        .call()
        .await?
        ._0;

    assert!(supports_interface);

    let erc721_interface_id: B32 = 0x80ac58cd_u32.into();
    let supports_interface =
        contract.supportsInterface(erc721_interface_id).call().await?._0;

    assert!(supports_interface);

    let erc165_interface_id: B32 = 0x01ffc9a7_u32.into();
    let supports_interface =
        contract.supportsInterface(erc165_interface_id).call().await?._0;

    assert!(supports_interface);

    let invalid_interface_id: B32 = 0xffffffff_u32.into();
    let supports_interface =
        contract.supportsInterface(invalid_interface_id).call().await?._0;

    assert!(!supports_interface);

    Ok(())
}
