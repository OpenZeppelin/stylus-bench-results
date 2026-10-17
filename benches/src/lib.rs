use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

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
    let workspace_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .context("benches package should be inside a workspace")?
        .to_path_buf();
    let contract_dir = workspace_dir.join("examples").join(contract_name);

    // Example configs opt into the workspace-pinned optimizer. Remove that
    // opt-in for the unoptimized passes, and leave it intact for the optimized
    // pass. The guard restores the exact original file after deployment.
    let _wasm_opt = match opt {
        Opt::CacheWasmOpt => Some(WasmOptConfig::enable(&contract_dir)?),
        Opt::None | Opt::Cache => Some(WasmOptConfig::disable(&contract_dir)?),
    };

    let mut deployer = account.as_deployer().with_directory(contract_dir);
    if let Some(constructor) = constructor {
        deployer = deployer.with_constructor(constructor);
    }
    let address = deployer.deploy().await?.contract_address;

    match opt {
        Opt::Cache | Opt::CacheWasmOpt => {
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

struct WasmOptConfig {
    path: PathBuf,
    original: Option<Vec<u8>>,
}

impl WasmOptConfig {
    fn enable(contract_dir: &Path) -> eyre::Result<Self> {
        let path = contract_dir.join("Stylus.toml");
        let original = path.exists().then(|| fs::read(&path)).transpose()?;
        let existing =
            original.as_deref().map(std::str::from_utf8).transpose()?;
        let existing = existing.unwrap_or_default();
        if !has_wasm_opt_table(existing) {
            let config = format!(
                "{existing}\n[wasm-opt]\nversion = \"116\"\nflags = [\"-O3\"]\n"
            );
            fs::write(&path, config)?;
        }

        Ok(Self { path, original })
    }

    fn disable(contract_dir: &Path) -> eyre::Result<Self> {
        let path = contract_dir.join("Stylus.toml");
        let original = path.exists().then(|| fs::read(&path)).transpose()?;
        let existing =
            original.as_deref().map(std::str::from_utf8).transpose()?;
        if let Some(existing) = existing {
            if let Some(disabled) = without_wasm_opt_table(existing) {
                fs::write(&path, disabled)?;
            }
        }

        Ok(Self { path, original })
    }
}

fn has_wasm_opt_table(contents: &str) -> bool {
    contents.lines().any(|line| table_header(line) == Some("[wasm-opt]"))
}

fn without_wasm_opt_table(contents: &str) -> Option<String> {
    let mut found = false;
    let mut in_wasm_opt = false;
    let mut output = String::with_capacity(contents.len());

    for line in contents.split_inclusive('\n') {
        if let Some(header) = table_header(line) {
            in_wasm_opt = header == "[wasm-opt]"
                || header == "[[wasm-opt]]"
                || header.starts_with("[wasm-opt.")
                || header.starts_with("[[wasm-opt.");
            found |= in_wasm_opt;
        }
        if !in_wasm_opt {
            output.push_str(line);
        }
    }

    found.then_some(output)
}

fn table_header(line: &str) -> Option<&str> {
    let header = line.split('#').next()?.trim();
    (header.starts_with('[') && header.ends_with(']')).then_some(header)
}

impl Drop for WasmOptConfig {
    fn drop(&mut self) {
        match self.original.take() {
            Some(contents) => {
                let _ = fs::write(&self.path, contents);
            }
            None => {
                let _ = fs::remove_file(&self.path);
            }
        }
    }
}

#[cfg(test)]
mod wasm_opt_config_tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;

    static NEXT_TEMP_DIRECTORY: AtomicU64 = AtomicU64::new(0);

    fn temp_contract_dir() -> PathBuf {
        let id = NEXT_TEMP_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir()
            .join(
                format!("stylus-bench-wasm-opt-{}-{id}", std::process::id(),),
            );
        fs::create_dir_all(&path)
            .expect("temporary contract directory should be created");
        path
    }

    #[test]
    fn disable_temporarily_removes_only_the_optimizer_table_and_restores_bytes()
    {
        let directory = temp_contract_dir();
        let path = directory.join("Stylus.toml");
        let original = b"# contract opts in\n[wasm-opt]\nversion = \"116\"\nflags = [\"-O3\"]\n[other]\nvalue = true\n";
        fs::write(&path, original).expect("config should be written");

        {
            let _guard = WasmOptConfig::disable(&directory).unwrap();
            let disabled = fs::read_to_string(&path)
                .expect("disabled config should be readable");
            assert!(!has_wasm_opt_table(&disabled));
            assert!(disabled.contains("[other]\nvalue = true"));
        }

        assert_eq!(fs::read(&path).unwrap(), original);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn enable_preserves_an_existing_optimizer_table() {
        let directory = temp_contract_dir();
        let path = directory.join("Stylus.toml");
        let original = b"# inherited recipe\n[wasm-opt]\n";
        fs::write(&path, original).expect("config should be written");

        {
            let _guard = WasmOptConfig::enable(&directory).unwrap();
            assert_eq!(fs::read(&path).unwrap(), original);
        }

        assert_eq!(fs::read(&path).unwrap(), original);
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn enable_temporary_config_is_removed_when_none_existed() {
        let directory = temp_contract_dir();
        let path = directory.join("Stylus.toml");

        {
            let _guard = WasmOptConfig::enable(&directory).unwrap();
            let enabled =
                fs::read_to_string(&path).expect("config should be created");
            assert!(has_wasm_opt_table(&enabled));
            assert!(enabled.contains("version = \"116\""));
        }

        assert!(!path.exists());
        fs::remove_dir_all(directory).unwrap();
    }
}
