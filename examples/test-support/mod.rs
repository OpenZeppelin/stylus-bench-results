//! Shared Alloy and Stylus SDK devnet support for example integration tests.

use std::{fmt::Debug, path::PathBuf};

use alloy::{
    network::{Ethereum, EthereumWallet, TransactionBuilder},
    primitives::{utils::parse_ether, Address, Bytes, U256},
    providers::{DynProvider, Provider, ProviderBuilder},
    rpc::types::{TransactionReceipt, TransactionRequest},
    signers::{local::PrivateKeySigner, Signature, Signer},
    sol_types::{SolError, SolEvent},
};
use eyre::{eyre, Result, WrapErr};
use stylus_tools::{devnet::Node, Activator, Deployer};
use tokio::sync::{Mutex, MutexGuard, OnceCell};

/// Alloy HTTP wallet provider used by the test accounts and Solidity mocks.
pub type Wallet = DynProvider<Ethereum>;

static DEVNET: OnceCell<Node> = OnceCell::const_new();
static TEST_LOCK: Mutex<()> = Mutex::const_new(());

async fn node() -> Result<&'static Node> {
    DEVNET.get_or_try_init(|| async { Node::new().await }).await
}

/// Holds the per-test lock and lazily starts one Nitro devnet per test binary.
///
/// Tests use the same devnet sequentially and deploy fresh contracts, which
/// keeps their state independent without starting a Docker container per test.
pub struct TestContext {
    _guard: MutexGuard<'static, ()>,
}

impl TestContext {
    /// Acquire the serial test lock and start the SDK devnet on first use.
    ///
    /// # Errors
    ///
    /// Returns an error if the devnet cannot start.
    pub async fn new() -> Result<Self> {
        let guard = TEST_LOCK.lock().await;
        node().await?;
        Ok(Self { _guard: guard })
    }
}

/// A funded local signer and provider connected to the test devnet.
#[derive(Clone)]
pub struct Account {
    /// Local signing key.
    pub signer: PrivateKeySigner,
    /// Alloy provider and wallet for this account.
    pub wallet: Wallet,
    rpc: String,
}

impl Account {
    /// Generate an account and fund it with 100 ETH from the devnet owner.
    ///
    /// # Errors
    ///
    /// Returns an error if the devnet request, funding transaction, or provider
    /// setup fails.
    pub async fn new() -> Result<Self> {
        let node = node().await?;
        let signer = PrivateKeySigner::random();
        let address = signer.address();
        let owner = node.create_provider().await?;
        owner
            .send_transaction(
                TransactionRequest::default()
                    .with_to(address)
                    .with_value(parse_ether("100")?),
            )
            .await?
            .watch()
            .await?;

        let wallet = ProviderBuilder::new()
            .wallet(EthereumWallet::from(signer.clone()))
            .connect_http(node.rpc().parse()?)
            .erased();

        Ok(Self { signer, wallet, rpc: node.rpc().to_owned() })
    }

    /// Return the account's private key as hex without a `0x` prefix.
    #[must_use]
    pub fn pk(&self) -> String {
        alloy::hex::encode(self.signer.to_bytes())
    }

    /// Return the account address.
    #[must_use]
    pub fn address(&self) -> Address {
        self.signer.address()
    }

    /// Return the RPC endpoint used by this account.
    #[must_use]
    pub fn url(&self) -> &str {
        &self.rpc
    }

    /// Sign a hash with the local account.
    ///
    /// # Panics
    ///
    /// Panics if the local signer cannot sign the hash.
    pub async fn sign_hash(&self, hash: &alloy::primitives::B256) -> Signature {
        self.signer.sign_hash(hash).await.expect("local signing should succeed")
    }

    /// Sign a message with the local account.
    ///
    /// # Panics
    ///
    /// Panics if the local signer cannot sign the message.
    pub async fn sign_message(&self, message: &[u8]) -> Signature {
        self.signer
            .sign_message(message)
            .await
            .expect("local signing should succeed")
    }

    /// Create a deployer for a Stylus example contract.
    #[must_use]
    pub fn as_deployer(&self) -> TestDeployer {
        TestDeployer {
            rpc: self.rpc.clone(),
            private_key: format!("0x{}", self.pk()),
            directory: PathBuf::from(env!("CARGO_MANIFEST_DIR")),
            constructor_args: None,
        }
    }
}

/// A contract receipt with the deployed Stylus address and deployment logs.
#[derive(Debug)]
pub struct Receipt {
    /// Receipt for the `StylusDeployer` transaction.
    pub inner: TransactionReceipt,
    /// Address of the deployed Stylus contract.
    pub contract_address: Address,
}

/// Stylus deployer wrapper for the example test fixtures.
pub struct TestDeployer {
    rpc: String,
    private_key: String,
    directory: PathBuf,
    constructor_args: Option<Vec<String>>,
}

impl TestDeployer {
    /// Set constructor arguments. The SDK reads the constructor signature from
    /// the selected crate's exported ABI.
    #[must_use]
    pub fn with_constructor(mut self, constructor: Constructor) -> Self {
        self.constructor_args = Some(constructor.args);
        self
    }

    /// Select a sibling example crate as the deployment target.
    ///
    /// # Panics
    ///
    /// Panics if the current manifest directory has no parent.
    #[must_use]
    pub fn with_example_name(mut self, example_name: &str) -> Self {
        self.directory = self
            .directory
            .parent()
            .expect("example manifest directory has a parent")
            .join(example_name);
        self
    }

    /// Select an explicit directory containing a Stylus contract crate.
    #[must_use]
    pub fn with_directory(mut self, directory: impl Into<PathBuf>) -> Self {
        self.directory = directory.into();
        self
    }

    /// Deploy and activate the configured Stylus example.
    ///
    /// # Errors
    ///
    /// Returns an error if deployment, activation, or receipt lookup fails.
    pub async fn deploy(self) -> Result<Receipt> {
        let directory = self.directory.to_string_lossy().into_owned();
        let deployment = match self.constructor_args {
            Some(args) => Deployer::builder()
                .rpc(self.rpc.clone())
                .private_key(self.private_key.clone())
                .dir(directory.clone())
                .constructor_args(args)
                .build()
                .deploy(),
            None => Deployer::builder()
                .rpc(self.rpc.clone())
                .private_key(self.private_key.clone())
                .dir(directory.clone())
                .build()
                .deploy(),
        }
        .map_err(map_initialization_error)?;

        let (contract_address, tx_hash, _) = deployment;
        Activator::builder()
            .rpc(self.rpc.clone())
            .private_key(self.private_key.clone())
            .contract_address(contract_address.to_string())
            .dir(directory)
            .build()
            .activate()?;

        let provider = ProviderBuilder::new()
            .wallet(EthereumWallet::from(
                self.private_key
                    .parse::<PrivateKeySigner>()
                    .wrap_err("invalid deployer private key")?,
            ))
            .connect_http(self.rpc.parse()?)
            .erased();
        let inner =
            provider.get_transaction_receipt(tx_hash).await?.ok_or_else(
                || eyre!("deployment transaction receipt was not found"),
            )?;

        Ok(Receipt { inner, contract_address })
    }
}

fn map_initialization_error(error: eyre::Report) -> eyre::Report {
    let initialization_selector = stylus_sdk::function_selector!(
        "ContractInitializationError",
        Address,
        Bytes
    );
    let selector = alloy::hex::encode(initialization_selector);
    let message = error.to_string().to_ascii_lowercase();
    if message.contains(&selector)
        || message.contains("contractinitializationerror")
    {
        eyre::Report::new(ContractInitializationError)
    } else {
        error
    }
}

/// Error returned by the Stylus deployer when a constructor reverts.
#[derive(Debug)]
pub struct ContractInitializationError;

impl std::fmt::Display for ContractInitializationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("ContractInitializationError")
    }
}

impl std::error::Error for ContractInitializationError {}

/// Constructor argument data used by [`TestDeployer`].
pub struct Constructor {
    /// Constructor signature retained for compatibility with old test helpers.
    pub signature: String,
    /// Stringified constructor values passed to `cargo stylus deploy`.
    pub args: Vec<String>,
}

/// Convert an ABI value to the format accepted by `cargo stylus deploy`.
pub trait AbiTypeToString {
    /// Convert the value to a deployer argument.
    fn abi_type_to_string(&self) -> String;
}

macro_rules! impl_to_arg_string {
    ($($ty:ty),* $(,)?) => {
        $(
            impl AbiTypeToString for $ty {
                fn abi_type_to_string(&self) -> String {
                    self.to_string()
                }
            }
        )*
    };
}

impl_to_arg_string!(Address, U256, u64, u8, String);

impl AbiTypeToString for alloy::primitives::aliases::U8 {
    fn abi_type_to_string(&self) -> String {
        self.to_string()
    }
}

impl AbiTypeToString for Bytes {
    fn abi_type_to_string(&self) -> String {
        format!("0x{}", alloy::hex::encode(self))
    }
}

macro_rules! constructor {
    () => {{
        $crate::test_support::Constructor { signature: "constructor()".to_owned(), args: vec![] }
    }};
    ($first:expr $(, $rest:expr)* $(,)?) => {{
        $crate::test_support::Constructor {
            // The SDK derives this signature from the selected package's ABI.
            signature: {
                fn abi_name<T: stylus_sdk::abi::AbiType>(_: T) -> &'static str {
                    T::ABI.as_str()
                }

                let params = vec![abi_name($first) $(, abi_name($rest))*];
                format!("constructor({})", params.join(","))
            },
            args: vec![
                $crate::test_support::AbiTypeToString::abi_type_to_string(&$first)
                $(, $crate::test_support::AbiTypeToString::abi_type_to_string(&$rest))*
            ],
        }
    }};
}

pub(crate) use constructor;

#[allow(unused_macros)]
macro_rules! send {
    ($expression:expr) => {{
        $expression.send().await
    }};
}

#[allow(unused_imports, unused_macros)]
pub(crate) use send;

#[allow(unused_macros)]
macro_rules! watch {
    ($expression:expr) => {{
        $expression.send().await?.watch().await
    }};
}

#[allow(unused_imports, unused_macros)]
pub(crate) use watch;

macro_rules! receipt {
    ($expression:expr) => {{
        $expression.send().await?.get_receipt().await
    }};
}

pub(crate) use receipt;

/// Assertion helpers for custom errors and emitted events.
pub trait Revert<E> {
    /// Check whether a failed call returned the given custom error.
    fn reverted_with(&self, expected: E) -> bool;
}

impl<E: SolError> Revert<E> for alloy::contract::Error {
    fn reverted_with(&self, expected: E) -> bool {
        self.as_revert_data()
            .is_some_and(|actual| actual.as_ref() == expected.abi_encode())
    }
}

impl<E: SolError> Revert<E> for eyre::Report {
    fn reverted_with(&self, expected: E) -> bool {
        self.chain()
            .find_map(|error| error.downcast_ref::<alloy::contract::Error>())
            .is_some_and(|error| error.reverted_with(expected))
    }
}

/// Solidity panic codes used by EVM-compatible panic payloads.
#[derive(Clone, Copy, Debug)]
#[repr(u8)]
pub enum PanicCode {
    /// Failed assertion.
    AssertionError = 0x01,
    /// Arithmetic overflow or underflow.
    ArithmeticOverflow = 0x11,
    /// Division by zero.
    DivisionByZero = 0x12,
    /// Invalid enum conversion.
    EnumConversionOutOfBounds = 0x21,
    /// Malformed storage byte array.
    IncorrectlyEncodedStorageByteArray = 0x22,
    /// Pop on an empty array.
    PopOnEmptyArray = 0x31,
    /// Out-of-bounds array access.
    ArrayAccessOutOfBounds = 0x32,
    /// Excessive memory allocation.
    TooMuchMemoryAllocated = 0x41,
    /// Called a zero-initialized internal function.
    ZeroInitializedVariable = 0x51,
}

/// Assertion helpers for Solidity panics.
pub trait Panic {
    /// Check whether a failed call returned this panic code.
    fn panicked_with(&self, expected: PanicCode) -> bool;
}

fn has_panic_code(error: &alloy::contract::Error, expected: PanicCode) -> bool {
    let Some(data) = error.as_revert_data() else { return false };
    if data.len() != 36 || data[..4] != [0x4e, 0x48, 0x7b, 0x71] {
        return false;
    }
    U256::from_be_slice(&data[4..]) == U256::from(expected as u8)
}

impl Panic for alloy::contract::Error {
    fn panicked_with(&self, expected: PanicCode) -> bool {
        has_panic_code(self, expected)
    }
}

impl Panic for eyre::Report {
    fn panicked_with(&self, expected: PanicCode) -> bool {
        self.chain()
            .find_map(|error| error.downcast_ref::<alloy::contract::Error>())
            .is_some_and(|error| error.panicked_with(expected))
    }
}

/// Check decoded events in receipts.
pub trait Ext<E> {
    /// Return whether this receipt contains the expected event.
    fn emits(&self, expected: E) -> bool;
}

impl<E> Ext<E> for TransactionReceipt
where
    E: SolEvent + PartialEq,
{
    fn emits(&self, expected: E) -> bool {
        self.inner
            .logs()
            .iter()
            .filter_map(|log| log.log_decode::<E>().ok())
            .map(|log| log.inner.data)
            .any(|event| event == expected)
    }
}

impl<E> Ext<E> for Receipt
where
    E: SolEvent + PartialEq,
{
    fn emits(&self, expected: E) -> bool {
        self.inner.emits(expected)
    }
}

pub use Ext as EventExt;
