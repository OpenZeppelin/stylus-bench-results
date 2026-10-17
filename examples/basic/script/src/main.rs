use alloy::{
    network::EthereumWallet,
    primitives::Address,
    providers::{Provider, ProviderBuilder, WalletProvider},
    signers::local::PrivateKeySigner,
    sol,
};
use cargo_metadata::MetadataCommand;
use stylus_test_support::{deploy_package, Constructor};

sol!(
    #[sol(rpc)]
    contract BasicToken {
        constructor(string memory name_, string memory symbol_);

        function name() external view returns (string name);
        function symbol() external view returns (string symbol);
    }
);

const RPC_URL: &str = "https://sepolia-rollup.arbitrum.io/rpc";
const PRIVATE_KEY: &str = "your private key";
const TOKEN_NAME: &str = "Test Token";
const TOKEN_SYMBOL: &str = "TTK";

#[tokio::main]
async fn main() {
    // WARNING: Please use a more secure method for storing your privaket key
    // than a string at the top of this file. The following code is for testing
    // purposes only.
    let signer = PRIVATE_KEY
        .parse::<PrivateKeySigner>()
        .expect("should parse the private key");
    let wallet = EthereumWallet::from(signer.clone());
    let rpc_url = RPC_URL.parse().expect("should parse rpc url");
    let provider = ProviderBuilder::new().wallet(wallet).connect_http(rpc_url);
    let contract_address =
        deploy(&provider).await.expect("contract should deploy");

    let contract = BasicToken::new(contract_address, &provider);

    let call_result = contract.name().call().await.unwrap();
    assert_eq!(call_result, TOKEN_NAME);

    let call_result = contract.symbol().call().await.unwrap();
    assert_eq!(call_result, TOKEN_SYMBOL);
}

/// Build, check, and deploy `basic-example` using SDK deployment helpers.
async fn deploy(
    provider: &(impl Provider + WalletProvider),
) -> eyre::Result<Address> {
    let metadata = MetadataCommand::new().exec()?;
    let package = metadata
        .packages
        .iter()
        .find(|package| package.name.as_str() == "basic-example")
        .expect("basic-example must be in the workspace");
    let manifest_dir = package
        .manifest_path
        .parent()
        .expect("package manifest has a parent directory")
        .as_std_path();
    std::env::set_current_dir(manifest_dir)?;
    let constructor = Constructor {
        signature: "constructor(string,string)".to_string(),
        args: vec![TOKEN_NAME.to_owned(), TOKEN_SYMBOL.to_owned()],
    };
    let receipt = deploy_package(package, provider, Some(&constructor)).await?;
    Ok(receipt.contract_address)
}
