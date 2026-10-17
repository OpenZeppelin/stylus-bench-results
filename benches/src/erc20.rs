use crate::test_support::{constructor, receipt, Account};
use alloy::{
    network::{AnyNetwork, EthereumWallet},
    primitives::Address,
    providers::ProviderBuilder,
    sol,
    sol_types::SolCall,
    uint,
};
use alloy_primitives::U256;

use crate::{
    report::{ContractReport, FunctionReport},
    Opt,
};

sol!(
    #[sol(rpc)]
    contract Erc20 {
        function name() external view returns (string name);
        function symbol() external view returns (string symbol);
        function decimals() external view returns (uint8 decimals);
        function totalSupply() external view returns (uint256 totalSupply);
        function balanceOf(address account) external view returns (uint256 balance);
        function allowance(address owner, address spender) external view returns (uint256 allowance);

        function cap() public view virtual returns (uint256 cap);

        function mint(address account, uint256 amount) external;
        function burn(uint256 amount) external;
        function burnFrom(address account, uint256 amount) external;

        function transfer(address recipient, uint256 amount) external returns (bool);
        function approve(address spender, uint256 amount) external returns (bool);
        function transferFrom(address sender, address recipient, uint256 amount) external returns (bool);
    }
);

const TOKEN_NAME: &str = "Test Token";
const TOKEN_SYMBOL: &str = "TTK";
const CAP: U256 = uint!(1_000_000_U256);

/// Run this contract benchmark in each cache and optimization mode.
///
/// # Errors
///
/// Returns an error if deployment, an RPC call, or receipt decoding fails.
pub async fn bench() -> eyre::Result<ContractReport> {
    Box::pin(ContractReport::generate("Erc20", run)).await
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
    let bob_wallet = ProviderBuilder::new_with_network::<AnyNetwork>()
        .wallet(EthereumWallet::from(bob.signer.clone()))
        .connect_http(bob.url().parse()?);

    let contract_addr = deploy(&alice, cache_opt).await?;

    let contract = Erc20::new(contract_addr, &alice_wallet);
    let contract_bob = Erc20::new(contract_addr, &bob_wallet);

    // IMPORTANT: Order matters!

    #[rustfmt::skip]
    let receipts = vec![
        (Erc20::nameCall::SIGNATURE, receipt!(contract.name())?),
        (Erc20::symbolCall::SIGNATURE, receipt!(contract.symbol())?),
        (Erc20::decimalsCall::SIGNATURE, receipt!(contract.decimals())?),
        (Erc20::totalSupplyCall::SIGNATURE, receipt!(contract.totalSupply())?),
        (Erc20::balanceOfCall::SIGNATURE, receipt!(contract.balanceOf(alice_addr))?),
        (Erc20::allowanceCall::SIGNATURE, receipt!(contract.allowance(alice_addr, bob_addr))?),
        (Erc20::capCall::SIGNATURE, receipt!(contract.cap())?),
        (Erc20::mintCall::SIGNATURE, receipt!(contract.mint(alice_addr, uint!(10_U256)))?),
        (Erc20::burnCall::SIGNATURE, receipt!(contract.burn(uint!(1_U256)))?),
        (Erc20::transferCall::SIGNATURE, receipt!(contract.transfer(bob_addr, uint!(1_U256)))?),
        (Erc20::approveCall::SIGNATURE, receipt!(contract.approve(bob_addr, uint!(5_U256)))?),
        (Erc20::burnFromCall::SIGNATURE, receipt!(contract_bob.burnFrom(alice_addr, uint!(1_U256)))?),
        (Erc20::transferFromCall::SIGNATURE, receipt!(contract_bob.transferFrom(alice_addr, bob_addr, uint!(4_U256)))?),
    ];

    receipts
        .into_iter()
        .map(|(signature, receipt)| FunctionReport::new(signature, &receipt))
        .collect::<eyre::Result<Vec<_>>>()
}

async fn deploy(account: &Account, cache_opt: Opt) -> eyre::Result<Address> {
    crate::deploy(
        account,
        "erc20",
        Some(constructor!(
            TOKEN_NAME.to_string(),
            TOKEN_SYMBOL.to_string(),
            CAP
        )),
        cache_opt,
    )
    .await
}
