use crate::test_support::{receipt, Account};
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
    contract Erc721 {
        function balanceOf(address owner) external view returns (uint256 balance);
        function approve(address to, uint256 tokenId) external;
        function getApproved(uint256 tokenId) external view returns (address approved);
        function isApprovedForAll(address owner, address operator) external view returns (bool approved);
        function ownerOf(uint256 tokenId) external view returns (address ownerOf);
        function safeTransferFrom(address from, address to, uint256 tokenId) external;
        function setApprovalForAll(address operator, bool approved) external;
        function totalSupply() external view returns (uint256 totalSupply);
        function transferFrom(address from, address to, uint256 tokenId) external;
        function mint(address to, uint256 tokenId) external;
        function burn(uint256 tokenId) external;
    }
);

/// Run this contract benchmark in each cache and optimization mode.
///
/// # Errors
///
/// Returns an error if deployment, an RPC call, or receipt decoding fails.
pub async fn bench() -> eyre::Result<ContractReport> {
    Box::pin(ContractReport::generate("Erc721", run)).await
}

/// Deploy the contract and run its benchmarked calls.
///
/// # Errors
///
/// Returns an error if account setup, deployment, an RPC call, or receipt decoding fails.
#[allow(clippy::large_futures)]
pub async fn run(cache_opt: Opt) -> eyre::Result<Vec<FunctionReport>> {
    let alice = Account::new().await?;
    let alice_addr = alice.address();
    let alice_wallet = ProviderBuilder::new_with_network::<AnyNetwork>()
        .wallet(EthereumWallet::from(alice.signer.clone()))
        .connect_http(alice.url().parse()?);

    let bob = Account::new().await?;
    let bob_addr = bob.address();

    let contract_addr = deploy(&alice, cache_opt).await?;

    let contract = Erc721::new(contract_addr, &alice_wallet);

    let token_1 = uint!(1_U256);
    let token_2 = uint!(2_U256);
    let token_3 = uint!(3_U256);
    let token_4 = uint!(4_U256);

    receipt!(contract.mint(alice_addr, token_2))?;
    receipt!(contract.mint(alice_addr, token_3))?;
    receipt!(contract.mint(alice_addr, token_4))?;

    // IMPORTANT: Order matters!

    #[rustfmt::skip]
    let receipts = vec![
        (Erc721::balanceOfCall::SIGNATURE, receipt!(contract.balanceOf(alice_addr))?),
        (Erc721::approveCall::SIGNATURE, receipt!(contract.approve(bob_addr, token_2))?),
        (Erc721::getApprovedCall::SIGNATURE, receipt!(contract.getApproved(token_2))?),
        (Erc721::isApprovedForAllCall::SIGNATURE, receipt!(contract.isApprovedForAll(alice_addr, bob_addr))?),
        (Erc721::ownerOfCall::SIGNATURE, receipt!(contract.ownerOf(token_2))?),
        (Erc721::safeTransferFromCall::SIGNATURE, receipt!(contract.safeTransferFrom(alice_addr, bob_addr, token_3))?),
        (Erc721::setApprovalForAllCall::SIGNATURE, receipt!(contract.setApprovalForAll(bob_addr, true))?),
        (Erc721::totalSupplyCall::SIGNATURE, receipt!(contract.totalSupply())?),
        (Erc721::transferFromCall::SIGNATURE, receipt!(contract.transferFrom(alice_addr, bob_addr, token_4))?),
        (Erc721::mintCall::SIGNATURE, receipt!(contract.mint(alice_addr, token_1))?),
        (Erc721::burnCall::SIGNATURE, receipt!(contract.burn(token_1))?),
    ];

    receipts
        .into_iter()
        .map(|(signature, receipt)| FunctionReport::new(signature, &receipt))
        .collect::<eyre::Result<Vec<_>>>()
}

async fn deploy(account: &Account, cache_opt: Opt) -> eyre::Result<Address> {
    crate::deploy(account, "erc721", None, cache_opt).await
}
