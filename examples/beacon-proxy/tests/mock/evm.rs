#![cfg(feature = "devnet")]

use alloy::{
    network::TransactionBuilder,
    primitives::{Address, Bytes, TxKind},
    providers::Provider,
    rpc::types::TransactionRequest,
};
use stylus_test_support::Wallet;

/// Deploy a tiny EVM contract that always returns the supplied address.
pub async fn deploy_constant_beacon(
    wallet: &Wallet,
    implementation: Address,
) -> eyre::Result<Address> {
    let mut runtime = vec![0x73]; // PUSH20
    runtime.extend_from_slice(implementation.as_slice());
    runtime
        .extend_from_slice(&[0x60, 0x00, 0x52, 0x60, 0x20, 0x60, 0x00, 0xf3]);
    deploy_runtime(wallet, &runtime).await
}

/// Deploy an EVM contract that reverts without data on every call.
pub async fn deploy_empty_reverter(wallet: &Wallet) -> eyre::Result<Address> {
    deploy_runtime(wallet, &[0x60, 0x00, 0x60, 0x00, 0xfd]).await
}

async fn deploy_runtime(
    wallet: &Wallet,
    runtime: &[u8],
) -> eyre::Result<Address> {
    let runtime_len = u8::try_from(runtime.len())?;
    let mut init_code = vec![
        0x60,
        runtime_len,
        0x60,
        0x0c, // creation-code header length
        0x60,
        0x00,
        0x39, // CODECOPY
        0x60,
        runtime_len,
        0x60,
        0x00,
        0xf3, // RETURN
    ];
    init_code.extend_from_slice(runtime);

    let mut transaction =
        TransactionRequest::default().with_input(Bytes::from(init_code));
    transaction.to = Some(TxKind::Create);
    let receipt =
        wallet.send_transaction(transaction).await?.get_receipt().await?;

    receipt
        .contract_address
        .ok_or_else(|| eyre::eyre!("runtime deployment returned no address"))
}
