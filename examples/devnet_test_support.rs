//! Shared support for the example contracts' devnet integration tests.
//!
//! Tests are ordinary Tokio tests and use Alloy clients against a local Nitro
//! devnode. Contracts are built with the workspace's pinned Rust and
//! Binaryen versions, then deployed with `cargo stylus`.
#![allow(dead_code, unused_imports, unused_macros)]

use std::{
    env,
    path::PathBuf,
    process::Command,
    str::FromStr,
    sync::{
        atomic::{AtomicU64, Ordering},
        LazyLock,
    },
};

use alloy::{
    network::{Ethereum, EthereumWallet},
    primitives::{Address, Bytes, TxHash, B256, U256},
    providers::{
        fillers::{
            BlobGasFiller, ChainIdFiller, FillProvider, GasFiller, JoinFill,
            NonceFiller, WalletFiller,
        },
        Identity, Provider, ProviderBuilder, RootProvider,
    },
    rpc::types::{TransactionReceipt, TransactionRequest},
    signers::{local::PrivateKeySigner, Signature, Signer},
    sol_types::{SolError, SolEvent},
    transports::{http::reqwest::Url, RpcError, TransportErrorKind},
};
use eyre::{Context, ContextCompat};
use stylus_sdk::function_selector;
use tokio::sync::Mutex;

const RPC_URL_ENV_VAR: &str = "RPC_URL";
const DEFAULT_RPC_URL: &str = "http://localhost:8547";
const DEFAULT_FUNDING_WEI: u64 = 100_000_000_000_000_000;
const DEPLOYER_FUNDING_WEI: u64 = 1_000_000_000_000_000_000;
pub const DEFAULT_DEPLOYER_ADDRESS: &str =
    "0xcEcba2F1DC234f70Dd89F2041029807F8D03A990";
/// Environment variable for the address of Nitro's Stylus deployer.
pub const DEPLOYER_ADDRESS: &str = "DEPLOYER_ADDRESS";
const CONTRACT_INITIALIZATION_ERROR_SELECTOR: [u8; 4] =
    function_selector!("ContractInitializationError", Address, Bytes);
const DEVNET_PRIVATE_KEY: &str =
    "0xb6b15c8cb491557369f3c7d2c287b053eb229daa9c22138887752191c9520659";
static FUNDING_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

/// Provider and wallet used by the example integration tests.
pub type Wallet = FillProvider<
    JoinFill<
        JoinFill<
            Identity,
            JoinFill<
                GasFiller,
                JoinFill<BlobGasFiller, JoinFill<NonceFiller, ChainIdFiller>>,
            >,
        >,
        WalletFiller<EthereumWallet>,
    >,
    RootProvider<Ethereum>,
    Ethereum,
>;

/// A funded transaction account for an E2E scenario.
#[derive(Clone, Debug)]
pub struct Account {
    /// The local signer used by this account.
    pub signer: PrivateKeySigner,
    /// Alloy provider with this account's wallet attached.
    pub wallet: Wallet,
    rpc_url: String,
}

impl Account {
    /// Create a fresh account and fund it from the Nitro devnode's public
    /// development key.
    pub async fn new() -> eyre::Result<Self> {
        let signer = PrivateKeySigner::random();
        let rpc_url = env::var(RPC_URL_ENV_VAR)
            .unwrap_or_else(|_| DEFAULT_RPC_URL.to_owned());
        let wallet = make_wallet(&signer, &rpc_url)?;

        // The devnode's single prefunded account funds the whole suite, so
        // give each isolated test account enough for several deployments and
        // transactions without spending the faucet balance on unused ETH.
        fund_address(&rpc_url, signer.address(), DEFAULT_FUNDING_WEI).await?;

        Ok(Self { signer, wallet, rpc_url })
    }

    /// Return the address controlled by this account.
    #[must_use]
    pub fn address(&self) -> Address {
        self.signer.address()
    }

    /// Return the RPC endpoint configured for this account.
    #[must_use]
    pub fn rpc_url(&self) -> &str {
        &self.rpc_url
    }

    /// Return this signer's hex-encoded private key.
    #[must_use]
    pub fn private_key(&self) -> String {
        alloy::hex::encode(self.signer.to_bytes())
    }

    /// Sign a 32-byte EIP-712 digest.
    pub async fn sign_hash(&self, hash: &B256) -> Signature {
        self.signer
            .sign_hash(hash)
            .await
            .expect("signer supports signing hashes")
    }

    /// Sign a message using the Ethereum personal-message prefix.
    pub async fn sign_message(&self, message: &[u8]) -> Signature {
        self.signer
            .sign_message(message)
            .await
            .expect("signer supports signing messages")
    }

    /// Create a cargo-stylus deployer for this account.
    #[must_use]
    pub fn as_deployer(&self) -> Deployer {
        Deployer {
            rpc_url: self.rpc_url.clone(),
            constructor: None,
            example_name: None,
        }
    }
}

fn make_wallet(
    signer: &PrivateKeySigner,
    rpc_url: &str,
) -> eyre::Result<Wallet> {
    Ok(ProviderBuilder::new()
        .wallet(EthereumWallet::from(signer.clone()))
        .connect_http(Url::from_str(rpc_url).context("invalid RPC_URL")?))
}

/// CLI-compatible arguments passed to `cargo stylus deploy`.
#[derive(Debug, Clone)]
pub struct Constructor {
    /// CLI-compatible values corresponding to the constructor arguments.
    pub args: Vec<String>,
}

impl Constructor {
    /// Build a constructor description from cargo-stylus CLI arguments.
    #[must_use]
    pub fn new(args: Vec<String>) -> Self {
        Self { args }
    }
}

/// Convert a constructor argument to the value format accepted by cargo-stylus.
pub trait AbiTypeToString {
    /// Convert this value into a single CLI constructor argument.
    fn abi_type_to_string(&self) -> String;
}

macro_rules! impl_display_arg {
    ($($ty:ty),+ $(,)?) => {$ (
        impl AbiTypeToString for $ty {
            fn abi_type_to_string(&self) -> String {
                self.to_string()
            }
        }
    )+ };
}

impl_display_arg!(Address, bool, String, U256, u8, u64);

impl AbiTypeToString for Bytes {
    fn abi_type_to_string(&self) -> String {
        format!("0x{}", alloy::hex::encode(self))
    }
}

/// Assert that a transaction succeeds and return its pending transaction.
macro_rules! send {
    ($call:expr) => {
        $call.send().await
    };
}
pub(crate) use send;

/// Send a transaction and wait for its receipt.
macro_rules! watch {
    ($call:expr) => {
        $crate::devnet_test_support::send!($call)?.watch().await
    };
}
pub(crate) use watch;

/// Send a transaction and retrieve its receipt.
macro_rules! receipt {
    ($call:expr) => {
        $crate::devnet_test_support::send!($call)?.get_receipt().await
    };
}
pub(crate) use receipt;

/// Build cargo-stylus constructor arguments.
macro_rules! constructor {
    () => {
        $crate::devnet_test_support::Constructor::new(Vec::new())
    };
    ($first:expr $(, $rest:expr)* $(,)?) => {{
        use $crate::devnet_test_support::AbiTypeToString as _;
        let first = $first;
        #[allow(unused_mut)]
        let mut args = vec![first.abi_type_to_string()];
        $(
            let arg = $rest;
            args.push(arg.abi_type_to_string());
        )*
        $crate::devnet_test_support::Constructor::new(args)
    }};
}
pub(crate) use constructor;

/// Error reported when a constructor reverts during Stylus deployment.
#[derive(Debug)]
pub struct ContractInitializationError;

impl ContractInitializationError {
    /// Downcast an error report to this constructor failure type.
    #[must_use]
    pub fn from_report(report: &eyre::Report) -> Option<&Self> {
        report.downcast_ref::<Self>()
    }
}

impl core::fmt::Display for ContractInitializationError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("contract constructor reverted")
    }
}

impl std::error::Error for ContractInitializationError {}

/// A receipt wrapper that records the Stylus contract address.
#[derive(Debug)]
pub struct Receipt {
    /// Receipt for the deployer transaction.
    pub inner: TransactionReceipt,
    /// Address of the deployed Stylus contract.
    pub contract_address: Address,
}

/// Extension trait for checking whether a receipt contains an event.
pub trait EventExt<E> {
    /// Return whether the receipt contains `expected`.
    fn emits(&self, expected: E) -> bool;
}

impl<E> EventExt<E> for TransactionReceipt
where
    E: SolEvent + PartialEq,
{
    fn emits(&self, expected: E) -> bool {
        self.inner
            .logs()
            .iter()
            .filter_map(|log| log.log_decode::<E>().ok())
            .any(|log| log.inner.data == expected)
    }
}

impl<E> EventExt<E> for Receipt
where
    E: SolEvent + PartialEq,
{
    fn emits(&self, expected: E) -> bool {
        self.inner.emits(expected)
    }
}

/// Solidity panic codes used by failure assertions.
#[derive(Clone, Copy, Debug)]
#[allow(missing_docs)]
pub enum PanicCode {
    AssertionError = 0x01,
    ArithmeticOverflow = 0x11,
    DivisionByZero = 0x12,
    EnumConversionOutOfBounds = 0x21,
    IncorrectlyEncodedStorageByteArray = 0x22,
    PopOnEmptyArray = 0x31,
    ArrayAccessOutOfBounds = 0x32,
    TooMuchMemoryAllocated = 0x41,
    ZeroInitializedVariable = 0x51,
}

/// Check whether a contract call trapped without an ABI revert payload.
pub trait Trap {
    /// Return whether the EVM reported an empty-data execution revert.
    fn trapped(&self) -> bool;
}

impl Trap for alloy::contract::Error {
    fn trapped(&self) -> bool {
        let Self::TransportError(error) = self else {
            return false;
        };
        let Some(payload) = error.as_error_resp() else {
            return false;
        };
        let empty_data = payload
            .data
            .as_ref()
            .is_some_and(|data| data.to_string().trim_matches('"') == "0x");
        payload.code == 3
            && payload.message == "execution reverted"
            && empty_data
    }
}

/// Check whether an Alloy contract error contains an expected Solidity error.
pub trait Revert<E> {
    /// Return whether this error contains the ABI-encoded `expected` error.
    fn reverted_with(&self, expected: E) -> bool;
}

impl<E: SolError> Revert<E> for alloy::contract::Error {
    fn reverted_with(&self, expected: E) -> bool {
        let Self::TransportError(error) = self else {
            return false;
        };
        let Some(payload) = error.as_error_resp() else {
            return false;
        };
        let Some(data) = payload.data.as_ref() else {
            return false;
        };
        let actual = data.to_string();
        let actual = actual.trim_matches('"').trim_start_matches("0x");
        let expected = alloy::hex::encode(expected.abi_encode());
        actual.eq_ignore_ascii_case(&expected)
    }
}

impl<E: SolError> Revert<E> for eyre::Report {
    fn reverted_with(&self, expected: E) -> bool {
        let Some(received) = self
            .chain()
            .find_map(|err| err.downcast_ref::<RpcError<TransportErrorKind>>())
        else {
            return false;
        };
        let RpcError::ErrorResp(received) = received else {
            return false;
        };
        received.data.as_ref().is_some_and(|data| {
            data.to_string()
                .contains(&alloy::hex::encode(expected.abi_encode()))
        })
    }
}

/// Configurable cargo-stylus deployer.
pub struct Deployer {
    rpc_url: String,
    constructor: Option<Constructor>,
    example_name: Option<String>,
}

impl Deployer {
    /// Add constructor arguments to this deployment.
    #[must_use]
    pub fn with_constructor(mut self, constructor: Constructor) -> Self {
        self.constructor = Some(constructor);
        self
    }

    /// Deploy the `*-example` package associated with a named example.
    #[must_use]
    pub fn with_example_name(mut self, example_name: &str) -> Self {
        self.example_name = Some(format!("{example_name}-example"));
        self
    }

    /// Deploy the current example through cargo-stylus.
    pub async fn deploy(&self) -> eyre::Result<Receipt> {
        let package_name = match &self.example_name {
            Some(name) => name.clone(),
            None => package_name_for_current_package()?,
        };
        self.deploy_package(&package_name).await
    }

    async fn deploy_package(
        &self,
        package_name: &str,
    ) -> eyre::Result<Receipt> {
        static NEXT_SALT: AtomicU64 = AtomicU64::new(1);

        // cargo-stylus runs in a separate process and cannot invalidate the
        // Alloy provider's cached nonce. Use a one-shot signer for deployment
        // transactions so later calls through the test account stay in sync.
        let deployer = PrivateKeySigner::random();
        fund_address(&self.rpc_url, deployer.address(), DEPLOYER_FUNDING_WEI)
            .await?;
        let private_key = alloy::hex::encode(deployer.to_bytes());

        let workspace = workspace_root()?;
        let contract_directory = example_directory(&workspace, package_name)?;

        let salt = NEXT_SALT.fetch_add(1, Ordering::Relaxed);
        let salt = format!("0x{:064x}", salt);
        let mut command = Command::new("cargo");
        command
            .args(["stylus", "deploy"])
            .arg("--no-verify")
            .args(["--contract", package_name])
            .args(["--endpoint", &self.rpc_url])
            .args(["--private-key", &private_key])
            .args([
                "--deployer-address",
                &env::var(DEPLOYER_ADDRESS)
                    .unwrap_or_else(|_| DEFAULT_DEPLOYER_ADDRESS.to_owned()),
            ])
            .args(["--deployer-salt", &salt])
            .env("CARGO_TERM_COLOR", "never");

        if let Some(constructor) = &self.constructor {
            command.arg("--constructor-args").args(&constructor.args);
        }

        let temporary_directory = workspace.join("target/stylus-tmp");
        std::fs::create_dir_all(&temporary_directory)?;
        let output = command
            // cargo-stylus builds the current Cargo package. Invoke it from
            // that package so workspace-wide host dependencies (for example
            // Alloy signers used by benches) do not enter the WASM build.
            .current_dir(contract_directory)
            // cargo-stylus creates its temporary Dockerfile under TMPDIR.
            // Keeping it inside the workspace avoids macOS file-sharing
            // failures when Docker BuildKit reads tempfile from /private/tmp.
            .env("TMPDIR", temporary_directory)
            .output()
            .context("failed to execute cargo stylus deploy")?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let output_text = format!("{stdout}\n{stderr}");
        if !output.status.success() {
            let details = &output_text;
            if self.constructor.is_some()
                && (details.contains("ContractInitializationError")
                    || contains_selector(
                        &details,
                        &CONTRACT_INITIALIZATION_ERROR_SELECTOR,
                    ))
            {
                return Err(eyre::Report::new(ContractInitializationError));
            }
            eyre::bail!("cargo stylus deploy failed:\n{details}");
        }

        let address = value_after_label(&output_text, "deployed code at address:")
            .ok_or_else(|| {
                eyre::eyre!(
                    "cargo stylus output did not contain a deployed address:\n{output_text}"
                )
            })?;
        let contract_address = Address::from_str(&address)?;
        let transaction_hash = value_after_label(
            &output_text,
            "deployment tx hash:",
        )
        .ok_or_else(|| {
            eyre::eyre!(
                "cargo stylus output did not contain a deployment transaction hash:\n{output_text}"
            )
        })?;
        let transaction_hash = TxHash::from_str(&transaction_hash)?;

        let provider =
            ProviderBuilder::new().connect_http(Url::from_str(&self.rpc_url)?);
        let inner = provider
            .get_transaction_receipt(transaction_hash)
            .await?
            .context("deployment transaction receipt not found")?;

        Ok(Receipt { inner, contract_address })
    }
}

async fn fund_address(
    rpc_url: &str,
    address: Address,
    amount_wei: u64,
) -> eyre::Result<()> {
    // The dev account is unlocked only through its known local key. Keep
    // funding serialized so the shared account's nonce cannot race when
    // Tokio runs multiple tests or deployments in parallel.
    let _guard = FUNDING_LOCK.lock().await;
    let funder = make_wallet(
        &PrivateKeySigner::from_str(
            &env::var("NITRO_DEVNODE_PRIVATE_KEY")
                .unwrap_or_else(|_| DEVNET_PRIVATE_KEY.to_owned()),
        )?,
        rpc_url,
    )?;
    if funder.get_balance(address).await? < U256::from(amount_wei) {
        funder
            .send_transaction(
                TransactionRequest::default()
                    .to(address)
                    .value(U256::from(amount_wei)),
            )
            .await?
            .get_receipt()
            .await?;
    }
    Ok(())
}

fn workspace_root() -> eyre::Result<PathBuf> {
    let mut directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    loop {
        let manifest = directory.join("Cargo.toml");
        if let Ok(contents) = std::fs::read_to_string(&manifest) {
            if contents.contains("[workspace]") {
                return Ok(directory);
            }
        }
        if !directory.pop() {
            eyre::bail!(
                "could not find workspace root from {}",
                manifest.display()
            );
        }
    }
}

fn value_after_label(output: &str, label: &str) -> Option<String> {
    output.lines().find_map(|line| {
        let value = line.split_once(label)?.1.trim();
        let value = value.split_whitespace().next()?;
        Some(strip_ansi_escape_sequences(value))
    })
}

fn strip_ansi_escape_sequences(value: &str) -> String {
    let mut output = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(character) = chars.next() {
        if character == '\u{1b}' {
            if chars.next() == Some('[') {
                for code in chars.by_ref() {
                    if ('@'..='~').contains(&code) {
                        break;
                    }
                }
            }
        } else {
            output.push(character);
        }
    }
    output
}

fn contains_selector(output: &str, selector: &[u8; 4]) -> bool {
    let bytes = output.as_bytes();
    for (index, prefix) in bytes.windows(2).enumerate() {
        if prefix != b"0x" {
            continue;
        }
        let start = index + 2;
        let end = bytes[start..]
            .iter()
            .position(|byte| !byte.is_ascii_hexdigit())
            .map_or(bytes.len(), |length| start + length);
        if end - start < 8 {
            continue;
        }
        if let Ok(data) = alloy::hex::decode(&output[start..end]) {
            if data.starts_with(selector) {
                return true;
            }
        }
    }
    false
}

fn package_name_for_current_package() -> eyre::Result<String> {
    let manifest_dir = env!("CARGO_MANIFEST_DIR");
    let manifest = std::fs::read_to_string(
        std::path::Path::new(manifest_dir).join("Cargo.toml"),
    )?;
    package_name_from_manifest(&manifest)
        .context("could not read package name from Cargo.toml")
}

fn example_directory(
    workspace: &std::path::Path,
    package_name: &str,
) -> eyre::Result<PathBuf> {
    let mut directories = vec![workspace.join("examples")];
    while let Some(directory) = directories.pop() {
        let manifest_path = directory.join("Cargo.toml");
        if let Ok(manifest) = std::fs::read_to_string(&manifest_path) {
            if package_name_from_manifest(&manifest).as_deref()
                == Some(package_name)
            {
                return Ok(directory);
            }
        }

        for entry in std::fs::read_dir(&directory)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                directories.push(entry.path());
            }
        }
    }

    eyre::bail!("could not find example package directory for {package_name}")
}

fn package_name_from_manifest(manifest: &str) -> Option<String> {
    manifest
        .lines()
        .skip_while(|line| line.trim() != "[package]")
        .skip(1)
        .take_while(|line| !line.trim_start().starts_with('['))
        .find_map(|line| {
            let (name, value) = line.split_once('=')?;
            (name.trim() == "name")
                .then(|| value.trim().trim_matches('"').to_owned())
        })
}
