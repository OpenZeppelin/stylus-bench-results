use std::{
    env, fs,
    path::{Path, PathBuf},
    sync::Mutex,
};

use alloy::{
    json_abi::Constructor as AbiConstructor,
    primitives::{Address, U256},
    providers::{Provider, WalletProvider},
};
use cargo_metadata::{MetadataCommand, Package};
use eyre::{bail, Context, Result};
use stylus_tools::{
    core::{
        activation,
        check::check_wasm_file,
        code::Code,
        deployment::{
            deployer::{get_address_from_receipt, parse_tx_calldata},
            fragments::deploy_fragments,
            request::DeploymentRequest,
            DeploymentConfig,
        },
        optimize::WasmOptConfig,
        project::{
            contract::{Contract, ContractStatus},
            hash_project,
        },
    },
    devnet::addresses::STYLUS_DEPLOYER,
};

// The SDK builder invokes Cargo relative to the current directory. Tests can
// deploy a sibling example from the workspace, so serialize the brief
// process-wide directory change around that synchronous build.
static BUILD_DIRECTORY_LOCK: Mutex<()> = Mutex::new(());

use crate::{account::Account, receipt::Receipt};

/// Constructor signature and values passed to the SDK deployment helper.
pub struct Constructor {
    /// Solidity-style constructor signature, for example `constructor(address)`.
    pub signature: String,
    /// Constructor values represented as strings for Alloy's dynamic ABI parser.
    pub args: Vec<String>,
}

/// Convert supported constructor arguments to the textual representation used by the SDK.
pub trait AbiTypeToString {
    /// Convert this ABI value to its constructor argument representation.
    fn abi_type_to_string(&self) -> String;
}

macro_rules! impl_to_arg_string {
    ($($abi_type:ty),* $(,)?) => {$ (
        impl AbiTypeToString for $abi_type {
            fn abi_type_to_string(&self) -> String {
                self.to_string()
            }
        }
    )*};
}

impl_to_arg_string!(
    U256,
    u64,
    String,
    stylus_sdk::alloy_primitives::U8,
    u8,
    Address
);

impl AbiTypeToString for stylus_sdk::abi::Bytes {
    fn abi_type_to_string(&self) -> String {
        format!("0x{}", alloy::hex::encode(self))
    }
}

/// Build a constructor descriptor using the ABI type information from Stylus SDK values.
#[macro_export]
macro_rules! constructor {
    () => {{
        $crate::Constructor {
            signature: "constructor()".to_string(),
            args: vec![],
        }
    }};
    ($first:expr $(, $rest:expr)* $(,)?) => {{
        fn get_abi_str<T: stylus_sdk::abi::AbiType>(_: T) -> &'static str {
            <T as stylus_sdk::abi::AbiType>::ABI.as_str()
        }

        let signature_params = {
            let mut params = vec![get_abi_str($first)];
            $(params.push(get_abi_str($rest));)*
            params.join(",")
        };
        let args = vec![
            $crate::AbiTypeToString::abi_type_to_string(&$first)
            $(, $crate::AbiTypeToString::abi_type_to_string(&$rest))*
        ];
        $crate::Constructor {
            signature: format!("constructor({signature_params})"),
            args,
        }
    }};
}

/// Error returned when contract initialization reverts in `StylusDeployer`.
#[derive(Debug)]
pub struct ContractInitializationError;

impl std::fmt::Display for ContractInitializationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ContractInitializationError")
    }
}

impl std::error::Error for ContractInitializationError {}

/// Direct Alloy deployment helper backed by the SDK's contract checker and devnet.
pub struct Deployer {
    account: Account,
    constructor: Option<Constructor>,
    package_name: Option<String>,
    build_features: Vec<String>,
}

impl Deployer {
    pub(crate) fn new(account: Account) -> Self {
        Self {
            account,
            constructor: None,
            package_name: None,
            build_features: Vec::new(),
        }
    }

    /// Set the constructor and arguments passed to `StylusDeployer`.
    #[must_use]
    pub fn with_constructor(mut self, constructor: Constructor) -> Self {
        self.constructor = Some(constructor);
        self
    }

    /// Select another workspace example package to deploy.
    #[must_use]
    pub fn with_example_name(mut self, name: &str) -> Self {
        self.package_name = Some(format!("{name}-example"));
        self
    }

    /// Add a Cargo feature when building the selected contract package.
    #[must_use]
    pub fn with_build_feature(mut self, feature: &str) -> Self {
        self.build_features.push(feature.to_owned());
        self
    }

    /// Build, check, deploy, and activate the selected contract.
    ///
    /// # Errors
    ///
    /// Returns an error if package discovery, SDK checking, deployment, or activation fails.
    pub async fn deploy(&self) -> Result<Receipt> {
        let package = find_package(self.package_name.as_deref())?;
        deploy_package_with_features(
            &package,
            &self.account.wallet,
            self.constructor.as_ref(),
            &self.build_features,
        )
        .await
    }
}

/// Deploy a package directly with an existing Alloy signer provider.
///
/// This helper lets examples target a custom RPC while integration tests use the SDK-managed
/// devnet provider.
///
/// # Errors
///
/// Returns an error if SDK validation, deployment, or activation fails.
pub async fn deploy_package(
    package: &Package,
    provider: &(impl Provider + WalletProvider),
    constructor: Option<&Constructor>,
) -> Result<Receipt> {
    deploy_package_with_features(package, provider, constructor, &[]).await
}

/// Build, check, deploy, and activate a package using the selected Cargo features.
///
/// # Errors
///
/// Returns an error if the SDK build, validation, deployment, or activation fails.
pub async fn deploy_package_with_features(
    package: &Package,
    provider: &(impl Provider + WalletProvider),
    constructor: Option<&Constructor>,
    features: &[String],
) -> Result<Receipt> {
    let contract = Contract::try_from(package).wrap_err_with(|| {
        format!("{} is not a deployable Stylus contract", package.name)
    })?;
    let mut config = DeploymentConfig {
        deployer_address: STYLUS_DEPLOYER,
        ..DeploymentConfig::default()
    };
    config.check.build.features = features.to_vec();
    let status = check_contract_for_devnet(&contract, &config, provider)
        .await
        .wrap_err("SDK contract check failed")?;
    deploy_status(status, &config, provider, constructor).await
}

/// Check a workspace contract through the SDK pipeline, with an extra WAT
/// round trip after optional optimization for the pinned Nitro devnet parser.
async fn check_contract_for_devnet(
    contract: &Contract,
    config: &DeploymentConfig,
    provider: &impl Provider,
) -> Result<ContractStatus> {
    let wasm_opt = WasmOptConfig::resolve_for_contract(contract)?;
    let package_dir = contract
        .package
        .manifest_path
        .parent()
        .ok_or_else(|| {
            eyre::eyre!("contract manifest has no parent directory")
        })?
        .as_std_path();
    let wasm_file =
        build_from_package_dir(contract, package_dir, &config.check.build)?;
    let project_hash = hash_project(
        package_dir,
        &config.check.project,
        &config.check.build,
        wasm_opt.as_ref(),
    )?;
    let raw_wasm = fs::read(&wasm_file)?;
    let optimized = stylus_tools::core::code::wasm::process_wasm(
        &raw_wasm,
        project_hash,
        wasm_opt.as_ref(),
    )?;

    // Nitro 3.11.1 rejects the optional DataCount section even when the module
    // uses only memory.copy/fill. SDK processing does an initial WAT round trip
    // before Binaryen; this second SDK round trip removes DataCount if Binaryen
    // adds it. Reprocessing the metadata is harmless because the checker strips
    // and writes the project hash itself.
    let wasm = stylus_tools::core::code::wasm::process_wasm(
        optimized.as_slice(),
        project_hash,
        None,
    )?;
    let temp_dir = tempfile::tempdir()?;
    let wasm_file = temp_dir.path().join("contract.wasm");
    fs::write(&wasm_file, wasm.as_slice())?;
    Ok(check_wasm_file(
        &wasm_file,
        project_hash,
        None,
        None,
        &config.check,
        provider,
    )
    .await?)
}

fn build_from_package_dir(
    contract: &Contract,
    package_dir: &Path,
    build: &stylus_tools::core::build::BuildConfig,
) -> Result<PathBuf> {
    let _lock = BUILD_DIRECTORY_LOCK
        .lock()
        .map_err(|_| eyre::eyre!("SDK build directory lock was poisoned"))?;
    let original_dir = env::current_dir()?;
    env::set_current_dir(package_dir)?;
    let restore = RestoreCurrentDir(original_dir);
    let result = contract.build(build);
    drop(restore);
    Ok(result?)
}

struct RestoreCurrentDir(PathBuf);

impl Drop for RestoreCurrentDir {
    fn drop(&mut self) {
        let _ = env::set_current_dir(&self.0);
    }
}

/// Deploy a previously built WASM file directly with an existing Alloy signer provider.
///
/// This is used by the gas benchmarks to compare the regular and wasm-opt optimized artifacts.
///
/// # Errors
///
/// Returns an error if the WASM file is invalid, cannot be deployed, or fails activation.
pub async fn deploy_wasm_file(
    wasm_file: impl AsRef<Path>,
    provider: &(impl Provider + WalletProvider),
    constructor: Option<&Constructor>,
) -> Result<Receipt> {
    let config = DeploymentConfig {
        deployer_address: STYLUS_DEPLOYER,
        ..DeploymentConfig::default()
    };
    let status = check_wasm_file(
        wasm_file,
        Default::default(),
        None,
        None,
        &config.check,
        provider,
    )
    .await
    .wrap_err("SDK WASM check failed")?;
    deploy_status(status, &config, provider, constructor).await
}

async fn deploy_status(
    status: ContractStatus,
    config: &DeploymentConfig,
    provider: &(impl Provider + WalletProvider),
    constructor: Option<&Constructor>,
) -> Result<Receipt> {
    let sender = provider.default_signer_address();
    let (receipt, contract_address) = if let Some(constructor) = constructor {
        let constructor_abi = AbiConstructor::parse(&constructor.signature)
            .wrap_err("invalid constructor signature")?;
        if constructor_abi.inputs.len() != constructor.args.len() {
            bail!(
                "constructor argument count mismatch: signature has {} inputs, got {} values",
                constructor_abi.inputs.len(),
                constructor.args.len()
            );
        }

        let code = code_for_deployment(&status, config, provider).await?;
        let calldata = parse_tx_calldata(
            &code,
            &constructor_abi,
            U256::ZERO,
            constructor.args.clone(),
            config.deployer_salt,
            provider,
        )
        .await
        .wrap_err("failed to encode Stylus constructor call")?;
        let request = DeploymentRequest::new_with_args(
            sender,
            config.deployer_address,
            status.suggest_fee(),
            calldata,
            config.max_fee_per_gas_gwei,
        );
        let receipt = request.exec(provider).await.map_err(|err| {
            if contains_initialization_error_selector(&err) {
                eyre::Report::new(ContractInitializationError)
            } else {
                eyre::Report::new(err)
            }
        })?;
        let address = get_address_from_receipt(&receipt)?;
        (receipt, address)
    } else {
        let was_active = matches!(&status, ContractStatus::Active { .. });
        let code = code_for_deployment(&status, config, provider).await?;
        let receipt =
            DeploymentRequest::new(sender, &code, config.max_fee_per_gas_gwei)
                .exec(provider)
                .await?;
        let address = receipt.contract_address.ok_or_else(|| {
            eyre::eyre!("deployment receipt did not include contract address")
        })?;
        if !was_active {
            activation::activate_contract(
                address,
                &config.check.activation,
                provider,
            )
            .await?;
        }
        (receipt, address)
    };

    Ok(Receipt { inner: receipt, contract_address })
}

async fn code_for_deployment(
    status: &ContractStatus,
    config: &DeploymentConfig,
    provider: &(impl Provider + WalletProvider),
) -> Result<Vec<u8>> {
    Ok(match status.code() {
        Code::Contract(code) => code.as_slice().to_vec(),
        Code::Fragments(fragments) => {
            let wasm_size = u32::try_from(status.wasm().len())
                .wrap_err("WASM binary size exceeds the deployment limit")?;
            deploy_fragments(fragments, wasm_size, config, provider).await?.0
        }
    })
}

fn find_package(package_name: Option<&str>) -> Result<Package> {
    let current_dir = std::env::current_dir()
        .wrap_err("cannot read test working directory")?;
    let metadata = MetadataCommand::new()
        .current_dir(&current_dir)
        .no_deps()
        .exec()
        .wrap_err("cannot read Cargo workspace metadata")?;
    let expected_name = match package_name {
        Some(name) => name.to_owned(),
        None => metadata
            .packages
            .iter()
            .find(|package| {
                package.manifest_path.as_std_path()
                    == current_dir.join("Cargo.toml")
            })
            .map(|package| package.name.to_string())
            .ok_or_else(|| {
                eyre::eyre!(
                    "no package manifest found in {}",
                    current_dir.display()
                )
            })?,
    };
    metadata
        .packages
        .into_iter()
        .find(|package| package.name.to_string() == expected_name)
        .ok_or_else(|| eyre::eyre!("Cargo package `{expected_name}` not found"))
}

fn contains_initialization_error_selector(
    error: &impl std::fmt::Debug,
) -> bool {
    let selector = stylus_sdk::function_selector!(
        "ContractInitializationError",
        Address,
        stylus_sdk::abi::Bytes
    );
    format!("{error:?}")
        .to_ascii_lowercase()
        .contains(&alloy::hex::encode(selector))
}
