use alloy::{
    primitives::Address,
    providers::{Provider, WalletProvider},
};

/// Place a bid to cache a deployed Stylus contract through the SDK's cache manager.
///
/// # Errors
///
/// Returns an error if the cache manager call fails.
pub async fn cache_contract(
    address: Address,
    bid: u128,
    provider: &(impl Provider + WalletProvider),
) -> eyre::Result<()> {
    let bid = alloy::primitives::U256::from_be_slice(&bid.to_be_bytes());
    stylus_tools::core::cache::place_bid(address, bid, None, provider).await?;
    Ok(())
}
