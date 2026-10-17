use benches::{
    access_control, erc1155, erc1155_metadata_uri, erc20, erc721,
    merkle_proofs, ownable, pedersen, poseidon, poseidon_asm_sol, poseidon_sol,
    report::BenchmarkReport,
};

#[tokio::main]
async fn main() -> eyre::Result<()> {
    // stylus-tools temporarily changes the process working directory while
    // invoking cargo-stylus, so benchmark deployments must be serialized.
    let reports = [
        access_control::bench().await?,
        erc20::bench().await?,
        erc721::bench().await?,
        merkle_proofs::bench().await?,
        ownable::bench().await?,
        erc1155::bench().await?,
        erc1155_metadata_uri::bench().await?,
        pedersen::bench().await?,
        poseidon_sol::bench().await?,
        poseidon_asm_sol::bench().await?,
        poseidon::bench().await?,
    ];
    let report = reports
        .into_iter()
        .fold(BenchmarkReport::default(), BenchmarkReport::merge_with);

    println!();
    println!("{report}");

    Ok(())
}
