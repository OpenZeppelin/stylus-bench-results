use crate::test_support::{constructor, receipt, Account};
use alloy::{
    network::{AnyNetwork, EthereumWallet},
    primitives::Address,
    providers::ProviderBuilder,
    sol,
    sol_types::SolCall,
    uint,
};

use crate::{
    report::{ContractReport, FunctionReport},
    Opt,
};

sol!(
    #[sol(rpc)]
    contract Erc1155MetadataUri {
        function uri(uint256 id) external view returns (string memory uri);
        function setTokenURI(uint256 tokenId, string memory tokenURI) external;
        function setBaseURI(string memory tokenURI) external;
    }
);

const URI: &str = "https://github.com/OpenZeppelin/rust-contracts-stylus";
const BASE_URI: &str = "https://github.com";
const TOKEN_URI: &str = "/some/token/uri";

/// Run this contract benchmark in each cache and optimization mode.
///
/// # Errors
///
/// Returns an error if deployment, an RPC call, or receipt decoding fails.
pub async fn bench() -> eyre::Result<ContractReport> {
    ContractReport::generate("Erc1155MetadataUri", run).await
}

/// Deploy the contract and run its benchmarked calls.
///
/// # Errors
///
/// Returns an error if account setup, deployment, an RPC call, or receipt decoding fails.
pub async fn run(cache_opt: Opt) -> eyre::Result<Vec<FunctionReport>> {
    let alice = Account::new().await?;
    let alice_wallet = ProviderBuilder::new_with_network::<AnyNetwork>()
        .wallet(EthereumWallet::from(alice.signer.clone()))
        .connect_http(alice.url().parse()?);

    let contract_addr = deploy(&alice, cache_opt).await?;

    let contract = Erc1155MetadataUri::new(contract_addr, &alice_wallet);

    let token_id = uint!(1_U256);

    // IMPORTANT: Order matters!

    #[rustfmt::skip]
    let receipts = vec![
        (Erc1155MetadataUri::setTokenURICall::SIGNATURE, receipt!(contract.setTokenURI(token_id, TOKEN_URI.to_owned()))?),
        (Erc1155MetadataUri::setBaseURICall::SIGNATURE, receipt!(contract.setBaseURI(BASE_URI.to_owned()))?),
        (Erc1155MetadataUri::uriCall::SIGNATURE, receipt!(contract.uri(token_id))?),
    ];

    receipts
        .into_iter()
        .map(|(signature, receipt)| FunctionReport::new(signature, &receipt))
        .collect::<eyre::Result<Vec<_>>>()
}

async fn deploy(account: &Account, cache_opt: Opt) -> eyre::Result<Address> {
    crate::deploy(
        account,
        "erc1155-metadata-uri",
        Some(constructor!(URI.to_string())),
        cache_opt,
    )
    .await
}
