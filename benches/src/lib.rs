#![allow(
    clippy::items_after_statements,
    clippy::large_futures,
    clippy::missing_errors_doc,
    clippy::wildcard_imports
)]

use alloy::primitives::Address;
use alloy_primitives::U128;
use eyre::WrapErr;
use serde::Deserialize;
use stylus_test_support::{
    cache_contract, deploy_wasm_file, Account, Constructor,
};

pub mod access_control;
pub mod erc1155;
pub mod erc1155_metadata_uri;
pub mod erc1155_supply;
pub mod erc20;
pub mod erc721;
pub mod merkle_proofs;
pub mod ownable;
pub mod pedersen;
pub mod poseidon;
pub mod poseidon_asm_sol;
pub mod poseidon_sol;
pub mod report;
pub mod vesting_wallet;

#[derive(Debug, Deserialize)]
struct ArbOtherFields {
    #[serde(rename = "gasUsedForL1")]
    gas_used_for_l1: U128,
    #[allow(dead_code)]
    #[serde(rename = "l1BlockNumber")]
    l1_block_number: String,
}

/// Optimisation options for the contract.
///
/// Cache or cache optimized WASM.
#[derive(Clone)]
pub enum Opt {
    None,
    Cache,
    CacheWasmOpt,
}

async fn deploy(
    account: &Account,
    contract_name: &str,
    constructor: Option<Constructor>,
    opt: Opt,
) -> eyre::Result<Address> {
    let manifest_dir =
        std::env::current_dir().context("should get current dir from env")?;

    let contract_type = match opt {
        Opt::CacheWasmOpt => "example_opt",
        Opt::None | Opt::Cache => "example",
    };

    let wasm_path = manifest_dir
        .join("target")
        .join("wasm32-unknown-unknown")
        .join("release")
        .join(format!(
            "{}_{}.wasm",
            contract_name.replace('-', "_"),
            contract_type
        ));

    let address =
        deploy_wasm_file(&wasm_path, &account.wallet, constructor.as_ref())
            .await?
            .contract_address;

    match opt {
        Opt::Cache | Opt::CacheWasmOpt => {
            cache_contract(address, 0, &account.wallet).await?;
        }
        Opt::None => {}
    }

    Ok(address)
}
