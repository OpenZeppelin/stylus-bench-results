#![cfg(feature = "devnet")]

use abi::{Erc721, Erc721Wrapper};
use alloy::primitives::{uint, Address, Bytes, U256};
use eyre::Result;
use stylus_test_support::{
    constructor, receipt, watch, Account, Constructor, EventExt, Revert,
};

mod abi;
mod mock;
use mock::{erc721, erc721::ERC721Mock};

fn ctr(asset_addr: Address) -> Constructor {
    constructor!(asset_addr)
}

async fn deploy(account: &Account) -> Result<(Address, Address)> {
    let asset_addr = erc721::deploy(&account.wallet).await?;

    let contract_addr = account
        .as_deployer()
        .with_constructor(ctr(asset_addr))
        .deploy()
        .await?
        .contract_address;

    Ok((asset_addr, contract_addr))
}

#[tokio::test]
async fn constructs() -> Result<()> {
    let alice = Account::new().await?;

    let asset_address = erc721::deploy(&alice.wallet).await?;
    let contract_addr = alice
        .as_deployer()
        .with_constructor(ctr(asset_address))
        .deploy()
        .await?
        .contract_address;
    let contract = Erc721Wrapper::new(contract_addr, alice.wallet);

    let underlying = contract.underlying().call().await?;
    assert_eq!(underlying, asset_address);

    Ok(())
}

#[tokio::test]
async fn deposit_for_success() -> Result<()> {
    let alice = Account::new().await?;

    let token_id = uint!(1_U256);
    let (asset_addr, contract_addr) = deploy(&alice).await?;
    let alice_address = alice.address();
    let asset = ERC721Mock::new(asset_addr, &alice.wallet);
    let contract = Erc721Wrapper::new(contract_addr, &alice.wallet);

    watch!(asset.safeMint(alice_address, token_id))?;
    watch!(asset.approve(contract_addr, token_id))?;

    let initial_alice_balance = asset.balanceOf(alice_address).call().await?;
    let initial_contract_balance =
        asset.balanceOf(contract_addr).call().await?;
    let initial_wrapped_balance =
        contract.balanceOf(alice_address).call().await?;

    let receipt = receipt!(contract.depositFor(alice_address, vec![token_id]))?;

    assert!(receipt.emits(Erc721::Transfer {
        from: alice_address,
        to: contract_addr,
        tokenId: token_id,
    }));

    assert!(receipt.emits(Erc721Wrapper::Transfer {
        from: Address::ZERO,
        to: alice_address,
        tokenId: token_id,
    }));

    let wrapped_owner = contract.ownerOf(token_id).call().await?;
    assert_eq!(wrapped_owner, alice_address);

    let underlying_owner = asset.ownerOf(token_id).call().await?;
    assert_eq!(underlying_owner, contract_addr);

    let one = uint!(1_U256);

    assert_eq!(
        initial_alice_balance - one,
        asset.balanceOf(alice_address).call().await?
    );
    assert_eq!(
        initial_contract_balance + one,
        asset.balanceOf(contract_addr).call().await?
    );
    assert_eq!(
        initial_wrapped_balance + one,
        contract.balanceOf(alice_address).call().await?
    );

    Ok(())
}

#[tokio::test]
async fn deposit_for_reverts_when_unsupported_token() -> Result<()> {
    let alice = Account::new().await?;
    let invalid_token = alice.address();
    let token_id = uint!(1_U256);
    let contract_addr = alice
        .as_deployer()
        .with_constructor(ctr(invalid_token))
        .deploy()
        .await?
        .contract_address;
    let contract = Erc721Wrapper::new(contract_addr, &alice.wallet);

    // A call to transferFrom on an EOA returns empty data without reverting. The wrapper must
    // reject that unsupported token instead of minting a wrapped token without collateral.
    match contract.depositFor(alice.address(), vec![token_id]).call().await {
        Err(error) => {
            assert!(error.reverted_with(
                Erc721Wrapper::Erc721FailedOperation { token: invalid_token }
            ));
            assert_eq!(
                contract.balanceOf(alice.address()).call().await?,
                U256::ZERO
            );
            assert!(contract.ownerOf(token_id).call().await.is_err());
        }
        Ok(result) => {
            let receipt =
                receipt!(contract.depositFor(alice.address(), vec![token_id]))?;
            let balance = contract.balanceOf(alice.address()).call().await?;
            let owner = contract.ownerOf(token_id).call().await?;
            eyre::bail!(
                "depositFor accepted an EOA underlying: returned {result:?}, minted balance {balance}, owner {owner}, tx {:?}",
                receipt.transaction_hash
            );
        }
    }

    Ok(())
}

#[tokio::test]
async fn on_erc721_received_reverts_for_unsupported_token() -> Result<()> {
    let alice = Account::new().await?;
    let invalid_token = alice.address();
    let token_id = uint!(2_U256);
    let contract_addr = alice
        .as_deployer()
        .with_constructor(ctr(invalid_token))
        .deploy()
        .await?
        .contract_address;
    let contract = Erc721Wrapper::new(contract_addr, &alice.wallet);

    assert_eq!(contract.balanceOf(alice.address()).call().await?, U256::ZERO);
    let err = contract
        .onERC721Received(
            invalid_token,
            alice.address(),
            token_id,
            Bytes::default(),
        )
        .call()
        .await
        .expect_err("an EOA cannot be the underlying ERC-721 token");
    assert!(err.reverted_with(abi::Erc721Wrapper::ERC721UnsupportedToken {
        token: invalid_token,
    }));
    assert_eq!(contract.balanceOf(alice.address()).call().await?, U256::ZERO);
    assert!(contract.ownerOf(token_id).call().await.is_err());

    Ok(())
}

#[tokio::test]
async fn withdraw_to_success() -> Result<()> {
    let alice = Account::new().await?;

    let token_id = uint!(1_U256);
    let (asset_addr, contract_addr) = deploy(&alice).await?;
    let alice_address = alice.address();
    let asset = ERC721Mock::new(asset_addr, &alice.wallet);
    let contract = Erc721Wrapper::new(contract_addr, &alice.wallet);

    watch!(asset.safeMint(alice_address, token_id))?;
    watch!(asset.approve(contract_addr, token_id))?;
    watch!(contract.depositFor(alice_address, vec![token_id]))?;

    let initial_alice_balance = asset.balanceOf(alice_address).call().await?;
    let initial_contract_balance =
        asset.balanceOf(contract_addr).call().await?;
    let initial_wrapped_balance =
        contract.balanceOf(alice_address).call().await?;

    let receipt = receipt!(contract.withdrawTo(alice_address, vec![token_id]))?;

    assert!(receipt.emits(Erc721Wrapper::Transfer {
        from: alice_address,
        to: Address::ZERO,
        tokenId: token_id,
    }));

    assert!(receipt.emits(Erc721::Transfer {
        from: contract_addr,
        to: alice_address,
        tokenId: token_id,
    }));

    let err = contract
        .ownerOf(token_id)
        .call()
        .await
        .expect_err("should return `ERC721NonexistentToken`");

    assert!(err.reverted_with(Erc721Wrapper::ERC721NonexistentToken {
        tokenId: token_id
    }));

    let underlying_owner = asset.ownerOf(token_id).call().await?;
    assert_eq!(underlying_owner, alice_address);

    let one = uint!(1_U256);

    assert_eq!(
        initial_alice_balance + one,
        asset.balanceOf(alice_address).call().await?
    );
    assert_eq!(
        initial_contract_balance - one,
        asset.balanceOf(contract_addr).call().await?
    );
    assert_eq!(
        initial_wrapped_balance - one,
        contract.balanceOf(alice_address).call().await?
    );

    Ok(())
}
