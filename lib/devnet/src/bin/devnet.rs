//! Starts a Stylus SDK devnet and keeps it running until Ctrl-C.
//!
//! Run `cargo run -p devnet --bin devnet`, then export the printed `RPC_URL`
//! before running end-to-end tests or benches. Requires Docker.

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let node = stylus_tools::devnet::Node::new().await?;
    println!("RPC_URL={}", node.rpc());
    tokio::signal::ctrl_c().await?;
    Ok(())
}
