use crate::test_support::{constructor, receipt, Account};
use alloy::{
    network::{AnyNetwork, EthereumWallet},
    primitives::Address,
    providers::ProviderBuilder,
    sol,
    sol_types::SolCall,
};

use crate::{
    report::{ContractReport, FunctionReport},
    Opt,
};

sol!(
    #[sol(rpc)]
    contract Ownable {
        function owner() external view returns (address owner);
        function renounceOwnership() external onlyOwner;
        function transferOwnership(address newOwner) external;
    }
);

/// Run this contract benchmark in each cache and optimization mode.
///
/// # Errors
///
/// Returns an error if deployment, an RPC call, or receipt decoding fails.
pub async fn bench() -> eyre::Result<ContractReport> {
    ContractReport::generate("Ownable", run).await
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

    let bob = Account::new().await?;
    let bob_addr = bob.address();
    let bob_wallet = ProviderBuilder::new_with_network::<AnyNetwork>()
        .wallet(EthereumWallet::from(bob.signer.clone()))
        .connect_http(bob.url().parse()?);

    let contract_addr = deploy(&alice, cache_opt).await?;

    let contract = Ownable::new(contract_addr, &alice_wallet);
    let contract_bob = Ownable::new(contract_addr, &bob_wallet);

    // IMPORTANT: Order matters!

    #[rustfmt::skip]
    let receipts = vec![
        (Ownable::ownerCall::SIGNATURE, receipt!(contract.owner())?),
        (Ownable::transferOwnershipCall::SIGNATURE, receipt!(contract.transferOwnership(bob_addr))?),
        (Ownable::renounceOwnershipCall::SIGNATURE, receipt!(contract_bob.renounceOwnership())?),
    ];

    receipts
        .into_iter()
        .map(|(signature, receipt)| FunctionReport::new(signature, &receipt))
        .collect::<eyre::Result<Vec<_>>>()
}

async fn deploy(account: &Account, cache_opt: Opt) -> eyre::Result<Address> {
    crate::deploy(
        account,
        "ownable",
        Some(constructor!(account.address())),
        cache_opt,
    )
    .await
}
