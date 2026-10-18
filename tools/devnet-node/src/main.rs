use std::io::{self, Write};

use alloy::{
    network::{EthereumWallet, TransactionBuilder},
    primitives::U256,
    providers::{Provider, ProviderBuilder},
    rpc::types::TransactionRequest,
    signers::local::PrivateKeySigner,
};
use stylus_tools::{
    devnet::{Node, DEVNET_PRIVATE_KEY},
    precompiles::{addresses::ARB_OWNER, ArbOwner},
};

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let node = Node::new().await?;

    // Keep activation estimates usable for large fragmented examples. Nitro's
    // default devnet base fee makes the SDK 0.10.10 activation simulation
    // require slightly more than its fixed 1 ETH probe value. Lowering the
    // L2 base fee keeps the SDK's own deploy/check path working; a priced
    // no-op transaction restores the RPC gas-price estimate used by the test
    // wallets while the block's base fee remains valid.
    let signer: PrivateKeySigner = DEVNET_PRIVATE_KEY.parse()?;
    let provider = ProviderBuilder::new()
        .wallet(EthereumWallet::from(signer.clone()))
        .connect_http(node.rpc().parse()?);
    let arb_owner = ArbOwner::new(ARB_OWNER, &provider);
    arb_owner
        .setMinimumL2BaseFee(U256::from(1))
        .gas_price(100_000_000)
        .send()
        .await?
        .watch()
        .await?;
    arb_owner
        .setL2BaseFee(U256::from(1))
        .gas_price(100_000_000)
        .send()
        .await?
        .watch()
        .await?;
    provider
        .send_transaction(
            TransactionRequest::default()
                .with_to(signer.address())
                .with_value(U256::ZERO)
                .with_gas_limit(21_000)
                .with_gas_price(100_000_000),
        )
        .await?
        .get_receipt()
        .await?;

    println!("RPC_URL={}", node.rpc());
    io::stdout().flush()?;

    #[cfg(unix)]
    {
        let mut terminate = tokio::signal::unix::signal(
            tokio::signal::unix::SignalKind::terminate(),
        )?;
        tokio::select! {
            result = tokio::signal::ctrl_c() => result?,
            _ = terminate.recv() => {},
        }
    }

    #[cfg(not(unix))]
    tokio::signal::ctrl_c().await?;

    drop(node);
    Ok(())
}
