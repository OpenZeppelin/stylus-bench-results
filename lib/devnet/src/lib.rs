//! Helpers for end-to-end tests and benches that run against the Stylus SDK
//! devnet.
//!
//! The devnet is provided by `stylus-tools` (a pinned Nitro node started in
//! Docker). Set `RPC_URL` to use a node that is already running; otherwise the
//! first call starts one for the current process.

use std::{
    path::{Path, PathBuf},
    sync::OnceLock,
};

use alloy::{
    network::{EthereumWallet, TransactionBuilder},
    primitives::{Address, Bytes, B256, U256},
    providers::{Provider, ProviderBuilder},
    rpc::types::TransactionRequest,
    signers::local::PrivateKeySigner,
    sol_types::SolCall,
};
use stylus_tools::{
    core::{
        check::{check_wasm_file, CheckConfig},
        deployment::{
            deployer::{get_address_from_receipt, stylus_constructorCall, StylusDeployer},
            prelude::DeploymentCalldata,
            request::DeploymentRequest,
        },
    },
    devnet::{addresses::STYLUS_DEPLOYER, DEVNET_PRIVATE_KEY},
};

/// Ether sent to each account created by [`funded_account`].
pub const DEFAULT_FUNDING_ETH: u64 = 10;

static RPC_URL: OnceLock<String> = OnceLock::new();

/// Serializes the funding transactions sent from the devnet owner key so that
/// their nonces do not collide when tests run in parallel.
static FUNDING: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Returns the RPC URL of the devnet used by this process.
///
/// The first call starts a devnet in a background thread that lives until the
/// process exits, unless `RPC_URL` is set. The devnet container is removed by
/// Docker when the process exits.
///
/// # Panics
///
/// Panics if the devnet cannot be started (for example when Docker is not
/// running).
#[must_use]
pub fn rpc_url() -> &'static str {
    RPC_URL
        .get_or_init(|| {
            if let Ok(url) = std::env::var("RPC_URL") {
                return url;
            }
            let (sender, receiver) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let runtime = tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()
                    .expect("failed to build the devnet runtime");
                runtime.block_on(async move {
                    let node = stylus_tools::devnet::Node::new()
                        .await
                        .expect("failed to start the devnet");
                    sender
                        .send(node.rpc().to_owned())
                        .expect("the test process is gone");
                    // Keep the node, and therefore its container, alive.
                    std::future::pending::<()>().await;
                });
            });
            receiver.recv().expect("the devnet did not start")
        })
        .as_str()
}

/// An externally owned account on the devnet.
#[derive(Clone, Debug)]
pub struct Account {
    signer: PrivateKeySigner,
    rpc: &'static str,
}

impl Account {
    /// Address of the account.
    #[must_use]
    pub fn address(&self) -> Address {
        self.signer.address()
    }

    /// Creates a provider that signs transactions with this account.
    ///
    /// # Errors
    ///
    /// Returns an error if the RPC endpoint cannot be reached.
    pub async fn provider(
        &self,
    ) -> eyre::Result<impl Provider + Clone + 'static> {
        let wallet = EthereumWallet::from(self.signer.clone());
        Ok(ProviderBuilder::new().wallet(wallet).connect(self.rpc).await?)
    }
}

/// Creates a new random account funded with [`DEFAULT_FUNDING_ETH`] ether
/// from the devnet owner.
///
/// # Errors
///
/// Returns an error if the funding transaction fails.
pub async fn funded_account() -> eyre::Result<Account> {
    fund(PrivateKeySigner::random(), DEFAULT_FUNDING_ETH).await
}

/// Creates a new random account funded with `eth` ether from the devnet owner.
///
/// # Errors
///
/// Returns an error if the funding transaction fails.
pub async fn fund(signer: PrivateKeySigner, eth: u64) -> eyre::Result<Account> {
    let rpc = rpc_url();
    let account = Account { signer, rpc };
    let owner: PrivateKeySigner = DEVNET_PRIVATE_KEY.parse()?;
    let owner_provider = ProviderBuilder::new()
        .wallet(EthereumWallet::from(owner))
        .connect(rpc)
        .await?;

    let _funding = FUNDING.lock().await;
    let value = U256::from(eth) * U256::from(10).pow(U256::from(18));
    let tx = TransactionRequest::default().with_to(account.address()).with_value(value);
    owner_provider.send_transaction(tx).await?.get_receipt().await?;
    Ok(account)
}

/// Returns the path of the release `.wasm` built for a contract crate.
///
/// The name is the crate name, for example `erc20-example`. Build the files
/// first with `cargo build --release --target wasm32-unknown-unknown`.
#[must_use]
pub fn wasm_path(crate_name: &str) -> PathBuf {
    let target = std::env::var_os("CARGO_TARGET_DIR").map_or_else(
        || Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target"),
        PathBuf::from,
    );
    target
        .join("wasm32-unknown-unknown/release")
        .join(format!("{}.wasm", crate_name.replace('-', "_")))
}

/// Deploys and activates a Stylus contract and returns its address.
///
/// `constructor_args` must be the ABI encoding of the constructor arguments,
/// for example `(name, symbol, cap).abi_encode_params()`, or `None` for a
/// contract without a constructor.
///
/// # Errors
///
/// Returns an error if the WASM cannot be checked, the contract is too large
/// for a single deployment, or the transaction fails.
pub async fn deploy(
    account: &Account,
    wasm: impl AsRef<Path>,
    constructor_args: Option<Bytes>,
) -> eyre::Result<Address> {
    let provider = account.provider().await?;
    let from = account.address();

    let status = check_wasm_file(
        wasm,
        Default::default(),
        None,
        None,
        &CheckConfig::default(),
        &provider,
    )
    .await?;
    let fee = status.suggest_fee();
    let code = match status.code() {
        stylus_tools::core::code::Code::Contract(code) => code.0.clone(),
        stylus_tools::core::code::Code::Fragments(_) => {
            eyre::bail!("fragmented contracts are not supported by the devnet helper")
        }
    };

    let receipt = match constructor_args {
        None => DeploymentRequest::new(from, &code, None).exec(&provider).await?,
        Some(args) => {
            let mut constructor = stylus_constructorCall::SELECTOR.to_vec();
            constructor.extend_from_slice(&args);
            let calldata = StylusDeployer::new(Address::ZERO, &provider)
                .deploy_call(
                    DeploymentCalldata::new(&code).into(),
                    constructor.into(),
                    U256::ZERO,
                    B256::ZERO,
                )
                .calldata()
                .to_vec();
            let receipt =
                DeploymentRequest::new_with_args(from, STYLUS_DEPLOYER, fee, calldata, None)
                    .exec(&provider)
                    .await?;
            return Ok(get_address_from_receipt(&receipt)?);
        }
    };
    receipt
        .contract_address
        .ok_or_else(|| eyre::eyre!("the deployment receipt has no contract address"))
}

