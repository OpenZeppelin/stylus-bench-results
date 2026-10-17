#![cfg(feature = "devnet")]

use abi::Erc721HolderExample;
use alloy::primitives::{Bytes, U256};
use eyre::Result;
use openzeppelin_stylus::token::erc721::RECEIVER_FN_SELECTOR;
use stylus_test_support::Account;

mod abi;

#[tokio::test]
async fn returns_correct_selector() -> Result<()> {
    let alice = Account::new().await?;

    let contract_addr = alice.as_deployer().deploy().await?.contract_address;

    let contract = Erc721HolderExample::new(contract_addr, &alice.wallet);

    let operator = alice.address();
    let from = alice.address();
    let token_id = U256::from(1);
    let data = Bytes::from(vec![0xde, 0xad, 0xbe, 0xef]);
    let interface_selector = contract
        .onERC721Received(operator, from, token_id, data)
        .call()
        .await?;

    assert_eq!(RECEIVER_FN_SELECTOR, interface_selector);

    Ok(())
}
