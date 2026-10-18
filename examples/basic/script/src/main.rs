use alloy::{
    network::EthereumWallet, primitives::Address, providers::ProviderBuilder,
    signers::local::PrivateKeySigner, sol,
};
use std::{path::PathBuf, process::Command};

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
    let contract_address = deploy(&signer).await;

    let contract = BasicToken::new(contract_address, &provider);

    let call_result = contract.name().call().await.unwrap();
    assert_eq!(call_result, TOKEN_NAME.to_owned());

    let call_result = contract.symbol().call().await.unwrap();
    assert_eq!(call_result, TOKEN_SYMBOL.to_owned());
}

/// Deploy a `BasicToken` contract to `RPC_URL` using `cargo-stylus`.
async fn deploy(signer: &PrivateKeySigner) -> Address {
    let workspace_root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let endpoint = RPC_URL;
    let private_key = format!("0x{}", alloy::hex::encode(signer.to_bytes()));
    let output = Command::new("cargo")
        .args(["stylus", "deploy"])
        .args(["--contract", "basic-example"])
        .args(["--endpoint", endpoint])
        .args(["--private-key", &private_key])
        .args(["--constructor-args", TOKEN_NAME, TOKEN_SYMBOL])
        .current_dir(workspace_root)
        .output()
        .expect("cargo-stylus should be installed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "contract deployment failed:\n{stdout}\n{stderr}"
    );
    let address = stdout
        .lines()
        .find_map(|line| {
            line.split_once("deployed code at address:")
                .map(|(_, value)| value.trim())
        })
        .and_then(|value| value.split_whitespace().next())
        .expect("cargo-stylus should print the deployed address");
    address.parse().expect("cargo-stylus should print a valid address")
}
