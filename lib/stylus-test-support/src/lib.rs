//! Shared helpers for integration tests that run against the Stylus SDK devnet.

mod account;
mod cache;
mod deploy;
mod error;
mod event;
mod receipt;

pub use account::{Account, Wallet};
pub use cache::cache_contract;
pub use deploy::{
    deploy_package, deploy_package_with_features, deploy_wasm_file,
    AbiTypeToString, Constructor, ContractInitializationError,
};
pub use error::{Panic, PanicCode, Revert};
pub use event::EventExt;
pub use receipt::Receipt;
pub use stylus_tools::devnet::addresses::STYLUS_DEPLOYER;

/// Sends a generated Alloy contract call.
#[macro_export]
macro_rules! send {
    ($call:expr) => {
        $call.send().await
    };
}

/// Sends a generated Alloy contract call and waits for its receipt.
#[macro_export]
macro_rules! watch {
    ($call:expr) => {
        $crate::send!($call)?.watch().await
    };
}

/// Sends a generated Alloy contract call and returns its mined receipt.
#[macro_export]
macro_rules! receipt {
    ($call:expr) => {
        $crate::send!($call)?.get_receipt().await
    };
}
