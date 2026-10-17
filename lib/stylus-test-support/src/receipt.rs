use alloy::{primitives::Address, rpc::types::TransactionReceipt};

/// Deployment receipt with the address of the Stylus program.
#[derive(Debug)]
pub struct Receipt {
    /// Receipt of the deployment transaction.
    pub inner: TransactionReceipt,
    /// Address of the deployed Stylus contract.
    pub contract_address: Address,
}
