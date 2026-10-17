use crate::test_support::{constructor, receipt, Account};
use alloy::{
    network::{AnyNetwork, EthereumWallet, TransactionBuilder},
    primitives::Address,
    providers::{Provider, ProviderBuilder},
    rpc::types::{
        serde_helpers::{OtherFields, WithOtherFields},
        TransactionRequest,
    },
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
    contract VestingWallet {
        function owner() public view virtual returns (address owner);
        function start() external view returns (uint256 start);
        function duration() external view returns (uint256 duration);
        function end() external view returns (uint256 end);
        function released() external view returns (uint256 released);
        function released(address token) external view returns (uint256 released);
        function releasable() external view returns (uint256 releasable);
        function releasable(address token) external view returns (uint256 releasable);
        function release() external;
        function release(address token) external;
        function vestedAmount(uint64 timestamp) external view returns (uint256 vestedAmount);
        function vestedAmount(address token, uint64 timestamp) external view returns (uint256 vestedAmount);
    }

    #[sol(rpc)]
    contract Erc20 {
        function mint(address account, uint256 amount) external;
    }
);

const START_TIMESTAMP: u64 = 1000;
const DURATION_SECONDS: u64 = 1000;

const TOKEN_NAME: &str = "Test Token";
const TOKEN_SYMBOL: &str = "TTK";
const CAP: U256 = uint!(1_000_000_U256);

/// Run this contract benchmark in each cache and optimization mode.
///
/// # Errors
///
/// Returns an error if deployment, an RPC call, or receipt decoding fails.
pub async fn bench() -> eyre::Result<ContractReport> {
    Box::pin(ContractReport::generate("VestingWallet", run)).await
}

/// Deploy the contract and run its benchmarked calls.
///
/// # Errors
///
/// Returns an error if account setup, deployment, an RPC call, or receipt decoding fails.
#[allow(clippy::large_futures)]
pub async fn run(cache_opt: Opt) -> eyre::Result<Vec<FunctionReport>> {
    let alice = Account::new().await?;
    let alice_wallet = ProviderBuilder::new_with_network::<AnyNetwork>()
        .wallet(EthereumWallet::from(alice.signer.clone()))
        .connect_http(alice.url().parse()?);

    let contract_addr = deploy(&alice, cache_opt.clone()).await?;
    let erc20_addr = deploy_token(&alice, cache_opt).await?;

    let contract = VestingWallet::new(contract_addr, &alice_wallet);
    let erc20 = Erc20::new(erc20_addr, &alice_wallet);

    let tx: WithOtherFields<TransactionRequest> = WithOtherFields {
        inner: TransactionRequest::default()
            .with_from(alice.address())
            .with_to(contract_addr)
            .with_value(uint!(1000_U256)),
        other: OtherFields::default(),
    };

    alice_wallet.send_transaction(tx).await?.watch().await?;
    receipt!(erc20.mint(contract_addr, uint!(1000_U256)))?;

    // IMPORTANT: Order matters!

    #[rustfmt::skip]
    let receipts = vec![
        (VestingWallet::startCall::SIGNATURE, receipt!(contract.start())?),
        (VestingWallet::durationCall::SIGNATURE, receipt!(contract.duration())?),
        (VestingWallet::endCall::SIGNATURE, receipt!(contract.end())?),
        (VestingWallet::released_0Call::SIGNATURE, receipt!(contract.released_0())?),
        (VestingWallet::released_1Call::SIGNATURE, receipt!(contract.released_1(erc20_addr))?),
        (VestingWallet::releasable_0Call::SIGNATURE, receipt!(contract.releasable_0())?),
        (VestingWallet::releasable_1Call::SIGNATURE, receipt!(contract.releasable_1(erc20_addr))?),
        (VestingWallet::release_0Call::SIGNATURE, receipt!(contract.release_0())?),
        (VestingWallet::release_1Call::SIGNATURE, receipt!(contract.release_1(erc20_addr))?),
        (VestingWallet::vestedAmount_0Call::SIGNATURE, receipt!(contract.vestedAmount_0(START_TIMESTAMP))?),
        (VestingWallet::vestedAmount_1Call::SIGNATURE, receipt!(contract.vestedAmount_1(erc20_addr, START_TIMESTAMP))?),
    ];

    receipts
        .into_iter()
        .map(|(signature, receipt)| FunctionReport::new(signature, &receipt))
        .collect::<eyre::Result<Vec<_>>>()
}

async fn deploy(account: &Account, cache_opt: Opt) -> eyre::Result<Address> {
    crate::deploy(
        account,
        "vesting-wallet",
        Some(constructor!(
            account.address(),
            START_TIMESTAMP,
            DURATION_SECONDS
        )),
        cache_opt,
    )
    .await
}

async fn deploy_token(
    account: &Account,
    cache_opt: Opt,
) -> eyre::Result<Address> {
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
