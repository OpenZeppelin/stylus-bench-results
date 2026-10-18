use alloy::{
    network::EthereumWallet, primitives::Address, providers::ProviderBuilder,
    signers::local::PrivateKeySigner, sol,
};
use std::path::PathBuf;
use stylus_tools::{Activator, Deployer};

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

    let contract_address = deploy(&signer);

    let contract = BasicToken::new(contract_address, &provider);

    let call_result = contract.name().call().await.unwrap();
    assert_eq!(call_result, TOKEN_NAME);

    let call_result = contract.symbol().call().await.unwrap();
    assert_eq!(call_result, TOKEN_SYMBOL);
}

/// Deploy a `BasicToken` contract to `RPC_URL` using `cargo-stylus`.
fn deploy(signer: &PrivateKeySigner) -> Address {
    let contract_dir =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../token");
    let private_key = format!("0x{}", alloy::hex::encode(signer.to_bytes()));

    let (address, _, _) = Deployer::builder()
        .rpc(RPC_URL)
        .private_key(private_key.clone())
        .dir(contract_dir.to_string_lossy().into_owned())
        .constructor_args(vec![TOKEN_NAME.to_owned(), TOKEN_SYMBOL.to_owned()])
        .build()
        .deploy()
        .expect("contract should be deployed");

    Activator::builder()
        .rpc(RPC_URL)
        .private_key(private_key)
        .contract_address(address.to_string())
        .dir(contract_dir.to_string_lossy().into_owned())
        .build()
        .activate()
        .expect("contract should be activated");

    address
}
