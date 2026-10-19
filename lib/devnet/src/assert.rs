//! Assertions for end-to-end tests.
//!
//! The helpers keep the strength of the assertions the v0.3.0 end-to-end
//! framework made: a revert must carry the exact ABI encoding of the expected
//! error, an event must decode to the expected value, and a panic is a
//! generic execution revert.

use alloy::{
    contract,
    primitives::Bytes,
    rpc::{json_rpc::RpcError, types::TransactionReceipt},
    sol_types::{SolError, SolEvent},
    transports::TransportErrorKind,
};

/// Returns the revert data that the node reported for a failed call.
///
/// Returns `None` when the error is not a node response or the node reported
/// no data, which is how Stylus panics are reported.
#[must_use]
pub fn revert_data(error: &contract::Error) -> Option<Bytes> {
    let contract::Error::TransportError(error) = error else {
        return None;
    };
    revert_data_of_rpc(error)
}

/// Returns whether a failed call reverted with exactly the ABI encoding of
/// `expected`.
#[must_use]
pub fn reverted_with<E: SolError>(error: &contract::Error, expected: &E) -> bool {
    revert_data(error).is_some_and(|data| data.as_ref() == expected.abi_encode())
}

/// Returns whether a failed call is a generic execution revert, which the
/// node uses for Stylus panics.
#[must_use]
pub fn is_execution_revert(error: &contract::Error) -> bool {
    let contract::Error::TransportError(error) = error else {
        return false;
    };
    error.as_error_resp().is_some_and(|payload| {
        payload.code == -32000 && payload.message == "execution reverted"
    })
}

/// Returns whether a deployment error, which carries the RPC error in its
/// chain, reverted with the ABI encoding of `expected`.
#[must_use]
pub fn deployment_reverted_with<E: SolError>(
    report: &eyre::Report,
    expected: &E,
) -> bool {
    let Some(error) = report
        .chain()
        .find_map(|cause| cause.downcast_ref::<RpcError<TransportErrorKind>>())
    else {
        return false;
    };
    revert_data_of_rpc(error)
        .is_some_and(|data| data.as_ref() == expected.abi_encode())
}

/// Returns whether the receipt contains an event that decodes to `expected`.
#[must_use]
pub fn emits<E>(receipt: &TransactionReceipt, expected: &E) -> bool
where
    E: SolEvent + PartialEq,
{
    receipt
        .inner
        .logs()
        .iter()
        .filter_map(|log| log.log_decode::<E>().ok())
        .any(|decoded| &decoded.inner.data == expected)
}

fn revert_data_of_rpc(error: &RpcError<TransportErrorKind>) -> Option<Bytes> {
    let payload = error.as_error_resp()?;
    let raw = payload.data.as_ref()?;
    let hex = raw.get().trim_matches('"');
    alloy::hex::decode(hex).ok().map(Bytes::from)
}
