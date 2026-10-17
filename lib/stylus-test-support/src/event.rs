use std::fmt::Debug;

use alloy::{rpc::types::TransactionReceipt, sol_types::SolEvent};

use crate::Receipt;

/// Assert that a receipt contains a matching event.
pub trait EventExt<E> {
    /// Return `true` if the receipt contains `expected`.
    fn emits(&self, expected: E) -> bool;
}

impl<E> EventExt<E> for TransactionReceipt
where
    E: SolEvent + PartialEq,
{
    fn emits(&self, expected: E) -> bool {
        self.inner
            .logs()
            .iter()
            .filter_map(|log| log.log_decode().ok())
            .any(|log| expected == log.inner.data)
    }
}

impl<E> EventExt<E> for Receipt
where
    E: SolEvent + PartialEq + Debug,
{
    fn emits(&self, expected: E) -> bool {
        self.inner.emits(expected)
    }
}
