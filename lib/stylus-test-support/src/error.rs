use alloy::sol_types::SolError;

/// EVM panic codes used by assertion helpers.
#[derive(Clone, Copy, Debug)]
#[allow(missing_docs)]
pub enum PanicCode {
    AssertionError = 0x1,
    ArithmeticOverflow = 0x11,
    DivisionByZero = 0x12,
    EnumConversionOutOfBounds = 0x21,
    IncorrectlyEncodedStorageByteArray = 0x22,
    PopOnEmptyArray = 0x31,
    ArrayAccessOutOfBounds = 0x32,
    TooMuchMemoryAllocated = 0x41,
    ZeroInitializedVariable = 0x51,
}

/// Check whether an Alloy error represents an EVM panic.
pub trait Panic {
    /// Check whether the error corresponds to the given panic code.
    fn panicked_with(&self, code: PanicCode) -> bool;
}

/// Check whether an Alloy or Eyre error includes a typed Solidity revert.
pub trait Revert<E> {
    /// Check whether the error contains the ABI encoding of `expected`.
    fn reverted_with(&self, expected: E) -> bool;
}

impl Panic for alloy::contract::Error {
    fn panicked_with(&self, code: PanicCode) -> bool {
        const PANIC_SELECTOR: [u8; 4] = [0x4e, 0x48, 0x7b, 0x71];

        let Some(data) = self.as_revert_data() else {
            return false;
        };
        if data.is_empty() {
            // Nitro devnet strips the Solidity panic selector and code from
            // Rust panics, returning only a generic execution-reverted
            // response. This matches the information available to the v0.3
            // E2E helper; the requested code cannot be verified in this case.
            return matches!(
                self,
                alloy::contract::Error::TransportError(error)
                    if error.as_error_resp().is_some_and(|payload| {
                        matches!(payload.code, -32000 | 3)
                            && payload.message == "execution reverted"
                    })
            );
        }
        if data.len() != 4 + 32 || data[..4] != PANIC_SELECTOR {
            return false;
        }

        let expected_code: [u8; 32] =
            alloy::primitives::U256::from(code as u64).to_be_bytes();
        data[4..] == expected_code
    }
}

impl<E: SolError> Revert<E> for alloy::contract::Error {
    fn reverted_with(&self, expected: E) -> bool {
        contains_hex(self, &expected.abi_encode())
    }
}

impl<E: SolError> Revert<E> for eyre::Report {
    fn reverted_with(&self, expected: E) -> bool {
        contains_hex(self, &expected.abi_encode())
    }
}

fn contains_hex(error: &impl std::fmt::Debug, data: &[u8]) -> bool {
    let rendered = format!("{error:?}").to_ascii_lowercase();
    rendered.contains(&alloy::hex::encode(data))
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use alloy::{
        contract::Error, rpc::json_rpc::ErrorPayload, transports::RpcError,
    };

    use super::{Panic, PanicCode};

    #[test]
    fn recognizes_nitro_revert_without_panic_data() {
        let payload = ErrorPayload {
            code: 3,
            message: Cow::Borrowed("execution reverted"),
            data: Some(serde_json::value::to_raw_value("0x").unwrap()),
        };
        let error = Error::TransportError(RpcError::ErrorResp(payload));

        assert!(error.panicked_with(PanicCode::ArithmeticOverflow));
    }

    #[test]
    fn checks_panic_code_when_revert_data_is_available() {
        let mut data = vec![0x4e, 0x48, 0x7b, 0x71];
        data.extend_from_slice(
            &alloy::primitives::U256::from(
                PanicCode::ArithmeticOverflow as u64,
            )
            .to_be_bytes::<32>(),
        );
        let payload = ErrorPayload {
            code: 3,
            message: Cow::Borrowed("execution reverted"),
            data: Some(
                serde_json::value::to_raw_value(&alloy::hex::encode_prefixed(
                    data,
                ))
                .unwrap(),
            ),
        };
        let error = Error::TransportError(RpcError::ErrorResp(payload));

        assert!(error.panicked_with(PanicCode::ArithmeticOverflow));
        assert!(!error.panicked_with(PanicCode::DivisionByZero));
    }
}
