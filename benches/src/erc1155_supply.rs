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
    contract Erc1155Supply {
        function mint(address to, uint256 id, uint256 amount, bytes memory data) external;
        function totalSupply(uint256 id) external view returns (uint256);
        function totalSupply() external view returns (uint256);
        function exists(uint256 id) external view returns (bool);
    }
);

/// Run this contract benchmark in each cache and optimization mode.
///
/// # Errors
///
/// Returns an error if deployment, an RPC call, or receipt decoding fails.
pub async fn bench() -> eyre::Result<ContractReport> {
    ContractReport::generate("Erc1155Supply", run).await
}

/// Deploy the contract and run its benchmarked calls.
///
/// # Errors
///
/// Returns an error if account setup, deployment, an RPC call, or receipt decoding fails.
pub async fn run(cache_opt: Opt) -> eyre::Result<Vec<FunctionReport>> {
    let alice = Account::new().await?;
    let alice_addr = alice.address();
    let alice_wallet = ProviderBuilder::new_with_network::<AnyNetwork>()
        .wallet(EthereumWallet::from(alice.signer.clone()))
        .connect_http(alice.url().parse()?);

    let contract_addr = deploy(&alice, cache_opt).await?;

    let contract = Erc1155Supply::new(contract_addr, &alice_wallet);

    let token = uint!(1_U256);
    let value = uint!(100_U256);

    // IMPORTANT: Order matters!

    #[rustfmt::skip]
    let receipts = vec![
        (Erc1155Supply::mintCall::SIGNATURE, receipt!(contract.mint(alice_addr, token, value, vec![].into()))?),
        (Erc1155Supply::existsCall::SIGNATURE, receipt!(contract.exists(token))?),
        (Erc1155Supply::totalSupply_0Call::SIGNATURE, receipt!(contract.totalSupply_0(token))?),
        (Erc1155Supply::totalSupply_1Call::SIGNATURE, receipt!(contract.totalSupply_1())?),
    ];

    receipts
        .into_iter()
        .map(|(signature, receipt)| FunctionReport::new(signature, &receipt))
        .collect::<eyre::Result<Vec<_>>>()
}

async fn deploy(account: &Account, cache_opt: Opt) -> eyre::Result<Address> {
    crate::deploy(account, "erc1155-supply", None, cache_opt).await
}
