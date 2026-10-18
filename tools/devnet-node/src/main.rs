use std::io::{self, Write};

use stylus_tools::devnet::Node;

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let node = Node::new().await?;

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
