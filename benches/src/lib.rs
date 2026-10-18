use std::{path::PathBuf, process::Command};

use crate::test_support::{Account, Constructor};
use alloy::primitives::Address;
use alloy_primitives::U128;
use eyre::{ContextCompat, WrapErr};
use serde::Deserialize;

#[path = "../../examples/test-support/mod.rs"]
pub mod test_support;

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

/// Benchmark modes supported by the SDK devnet.
///
/// Binaryen 116 output currently contains a `DataCountSection` rejected by the
/// SDK devnet's Nitro parser, so benchmarks use the deployable Rust Wasm output.
#[derive(Clone)]
pub enum Opt {
    None,
    Cache,
}

async fn deploy(
    account: &Account,
    contract_name: &str,
    constructor: Option<Constructor>,
    opt: Opt,
) -> eyre::Result<Address> {
    let workspace_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .context("benches package should be inside a workspace")?
        .to_path_buf();
    let contract_dir = workspace_dir.join("examples").join(contract_name);

    let mut deployer = account.as_deployer().with_directory(contract_dir);
    if let Some(constructor) = constructor {
        deployer = deployer.with_constructor(constructor);
    }
    let address = deployer.deploy().await?.contract_address;

    match opt {
        Opt::Cache => {
            cache_contract(account, address, 0)?;
        }
        Opt::None => {}
    }

    Ok(address)
}

/// Try to cache a contract on the stylus network.
/// Already cached contracts won't be cached, and this function will not return
/// an error.
/// Output will be forwarded to the child process.
fn cache_contract(
    account: &Account,
    contract_addr: Address,
    bid: u32,
) -> eyre::Result<()> {
    // We don't need a status code.
    // Since it is not zero when the contract is already cached.
    Command::new("cargo")
        .args(["stylus", "cache", "bid"])
        .args(["-e", account.url()])
        .args(["--private-key", &format!("0x{}", account.pk())])
        .arg(contract_addr.to_string())
        .arg(bid.to_string())
        .status()
        .context("failed to execute `cargo stylus cache bid` command")?;
    Ok(())
}
