use alloy::{
    network::{Ethereum, EthereumWallet, TransactionBuilder},
    primitives::{Address, B256},
    providers::{
        fillers::{FillProvider, JoinFill, WalletFiller},
        utils::JoinedRecommendedFillers,
        Provider, ProviderBuilder, RootProvider,
    },
    signers::{local::PrivateKeySigner, Signature, Signer},
};
use eyre::Context;
use std::sync::{Arc, Weak};
use stylus_tools::devnet::Node;
use tokio::sync::Mutex;

use crate::deploy::Deployer;

const DEFAULT_FUNDING_ETH: &str = "1";

/// An Alloy provider configured with recommended transaction fillers and a local signer.
pub type Wallet = FillProvider<
    JoinFill<JoinedRecommendedFillers, WalletFiller<EthereumWallet>>,
    RootProvider,
    Ethereum,
>;

/// A funded test account on the SDK-managed Nitro devnet.
#[derive(Clone)]
pub struct Account {
    /// This account's local private key wrapper.
    pub signer: PrivateKeySigner,
    /// This account's Alloy provider and signer.
    pub wallet: Wallet,
    rpc_url: String,
    // Keep the SDK-managed devnet alive exactly as long as accounts use it.
    // A process-global strong `Node` never gets dropped, leaving Nitro
    // containers running after test binaries exit.
    _node: Arc<Node>,
}

impl std::fmt::Debug for Account {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Account")
            .field("signer", &self.signer)
            .field("rpc_url", &self.rpc_url)
            .finish_non_exhaustive()
    }
}

impl Account {
    /// Create and fund a fresh account on the shared devnet for this test binary.
    ///
    /// # Errors
    ///
    /// Returns an error if the SDK devnet cannot start or account funding fails.
    pub async fn new() -> eyre::Result<Self> {
        let node = {
            let mut devnet = DEVNET.lock().await;
            if let Some(node) = devnet.upgrade() {
                node
            } else {
                let node = Arc::new(
                    Node::new()
                        .await
                        .wrap_err("failed to start Stylus SDK devnet")?,
                );
                *devnet = Arc::downgrade(&node);
                node
            }
        };
        let signer = PrivateKeySigner::random();
        let address = signer.address();

        // Every test account is funded by the devnet owner. Serialize these transfers so the
        // owner provider's nonce filler cannot race when tests execute concurrently.
        let _guard = FUNDING_LOCK.lock().await;
        let owner = node.create_provider().await?;
        let amount =
            alloy::primitives::utils::parse_ether(DEFAULT_FUNDING_ETH)?;
        owner
            .send_transaction(
                alloy::rpc::types::TransactionRequest::default()
                    .with_to(address)
                    .with_value(amount),
            )
            .await?
            .get_receipt()
            .await?;

        let rpc_url = node.rpc().to_owned();
        let rpc = rpc_url.parse().context("invalid devnet RPC URL")?;
        let wallet = ProviderBuilder::new()
            .wallet(EthereumWallet::from(signer.clone()))
            .connect_http(rpc);

        Ok(Self { signer, wallet, rpc_url, _node: node })
    }

    /// Return the private key as a hexadecimal string.
    #[must_use]
    pub fn pk(&self) -> String {
        alloy::hex::encode(self.signer.to_bytes())
    }

    /// Return this account's address.
    #[must_use]
    pub fn address(&self) -> Address {
        self.signer.address()
    }

    /// Return the endpoint used by this account's provider.
    #[must_use]
    pub fn url(&self) -> &str {
        &self.rpc_url
    }

    /// Sign a hash with this account.
    ///
    /// # Panics
    ///
    /// Panics if the local signer cannot sign the supplied hash.
    pub async fn sign_hash(&self, hash: &B256) -> Signature {
        self.signer
            .sign_hash(hash)
            .await
            .expect("local signer should sign hashes")
    }

    /// Sign a message with this account.
    ///
    /// # Panics
    ///
    /// Panics if the local signer cannot sign the supplied message.
    pub async fn sign_message(&self, message: &[u8]) -> Signature {
        self.signer
            .sign_message(message)
            .await
            .expect("local signer should sign messages")
    }

    /// Create a deployer for this account.
    #[must_use]
    pub fn as_deployer(&self) -> Deployer {
        Deployer::new(self.clone())
    }
}

static DEVNET: Mutex<Weak<Node>> = Mutex::const_new(Weak::new());
static FUNDING_LOCK: Mutex<()> = Mutex::const_new(());
