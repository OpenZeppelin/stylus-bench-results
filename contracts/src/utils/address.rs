//! A collection of utilities for working with [`Address`].

use alloc::vec::Vec;

use alloy_primitives::Address;
pub use sol::*;
use stylus_sdk::{
    call,
    host::VM,
    prelude::*,
    stylus_core::calls::errors::{Error as CallError, MethodError},
};

#[cfg_attr(coverage_nightly, coverage(off))]
mod sol {
    use alloy_sol_macro::sol;

    sol! {
        /// There's no code at `target` (it is not a contract).
        ///
        /// * `target` - Address of the target contract.
        #[derive(Debug)]
        #[allow(missing_docs)]
        error AddressEmptyCode(address target);

        /// A call to an address target failed. The target may have reverted
        /// without a revert reason.
        #[derive(Debug)]
        #[allow(missing_docs)]
        error FailedCall();

        /// A call to an address target failed. The target may have reverted
        /// with a reason.
        ///
        /// * `reason` - The revert reason that was returned by the call.
        #[derive(Debug)]
        #[allow(missing_docs)]
        error FailedCallWithReason(bytes reason);
    }
}

/// An [`AddressUtils`] error.
#[derive(SolidityError, Debug)]
pub enum Error {
    /// There's no code at `target` (it is not a contract).
    EmptyCode(AddressEmptyCode),
    /// A call to an address target failed. The target may have reverted
    /// without a revert reason.
    FailedCall(FailedCall),
    /// A call to an address target failed. The target may have reverted
    /// with a reason.
    FailedCallWithReason(FailedCallWithReason),
}

impl MethodError for Error {
    fn encode(self) -> alloc::vec::Vec<u8> {
        self.into()
    }
}

/// A collection of utilities for working with [`Address`].
pub struct AddressUtils;

impl AddressUtils {
    /// Performs a delegate call to `target` with the given `data`.
    ///
    /// # Arguments
    ///
    /// * `context` - Mutable access to the contract's state and to its host.
    /// * `target` - The address of the target contract.
    /// * `data` - The data to pass to the target contract.
    ///
    /// # Errors
    ///
    /// * [`Error::FailedCall`] - If the call to the target contract fails
    ///   without a revert reason.
    /// * [`Error::FailedCallWithReason`] - If the call to the target contract
    ///   fails with a revert reason or if the call fails for any other reason.
    /// * [`Error::EmptyCode`] - If the target contract has no code.
    pub fn function_delegate_call(
        context: &mut (impl TopLevelStorage + HostAccess<Host = VM>),
        target: Address,
        data: &[u8],
    ) -> Result<Vec<u8>, Error> {
        // The call context borrows `context` mutably, so it must be built
        // before the host is accessed through `context.vm()`.
        let call = Call::new_mutating(context);
        let result =
            unsafe { call::delegate_call(context.vm(), call, target, data) };
        Self::verify_call_result_from_target(context, target, result)
    }

    // TODO: Support more result types out of the box (e.g. `U256`, `U160`,
    // `String`, etc.).
    /// Helper function to verify that a low level call to smart-contract was
    /// successful.
    ///
    /// Reverts if the target was not a contract or if the call fails for any
    /// other reason. Bubbles up the revert reason (falling back to
    /// [`Error::FailedCall`]) in case of an unsuccessful call.
    ///
    /// # Arguments
    ///
    /// * `context` - Read access to the contract's host, used to look up the
    ///   code of `target`.
    /// * `target` - The address of the target contract.
    /// * `result` - The result of the call.
    ///
    /// # Errors
    ///
    /// * [`Error::EmptyCode`] - If the target contract has no code.
    /// * [`Error::FailedCallWithReason`] - If the call to the target contract
    ///   fails with a revert reason or if the call fails for any other reason.
    /// * [`Error::FailedCall`] - If the call to the target contract fails
    ///   without a revert reason.
    pub fn verify_call_result_from_target<T: AsRef<[u8]>>(
        context: &impl HostAccess<Host = VM>,
        target: Address,
        result: Result<T, CallError>,
    ) -> Result<T, Error> {
        match result {
            Ok(returndata) => {
                if returndata.as_ref().is_empty()
                    && !Self::has_code(context, target)
                {
                    return Err(AddressEmptyCode { target }.into());
                }
                Ok(returndata)
            }
            Err(e) => Err(Self::revert(e)),
        }
    }

    /// Returns `true` if there is code deployed at `account`, and `false`
    /// otherwise.
    ///
    /// # Arguments
    ///
    /// * `context` - Read access to the contract's host.
    /// * `account` - The address to look up the code of.
    pub(crate) fn has_code(
        context: &impl HostAccess<Host = VM>,
        account: Address,
    ) -> bool {
        context.vm().code_size(account) > 0
    }
}

impl AddressUtils {
    /// Reverts with `error` if revert reason exists. Otherwise reverts with
    /// [`Error::FailedCall`].
    ///
    /// This behavior is aligned with Solidity implementation of
    /// [Address.sol].
    ///
    /// [Address.sol]: https://github.com/OpenZeppelin/openzeppelin-contracts/blob/master/contracts/utils/Address.sol
    fn revert(error: CallError) -> Error {
        match &error {
            CallError::Revert(data) if data.is_empty() => FailedCall {}.into(),
            _ => FailedCallWithReason { reason: error.encode().into() }.into(),
        }
    }
}

#[cfg(test)]
mod tests {
    use stylus_sdk::testing::TestVM;

    use super::*;
    use crate::test_utils::BOB;

    #[storage]
    struct Caller;

    unsafe impl TopLevelStorage for Caller {}

    #[test]
    fn has_code_returns_false_for_account_without_code() {
        let vm = TestVM::default();
        let contract = Caller::from(&vm);

        assert!(!AddressUtils::has_code(&contract, BOB));
    }

    #[test]
    fn has_code_returns_true_for_account_with_code() {
        let vm = TestVM::default();
        let contract = Caller::from(&vm);
        vm.set_code(BOB, vec![0x60, 0x80]);

        assert!(AddressUtils::has_code(&contract, BOB));
    }

    #[test]
    fn function_delegate_call_returns_the_returned_data() {
        let vm = TestVM::default();
        let mut contract = Caller::from(&vm);
        vm.mock_delegate_call(BOB, vec![1, 2, 3], Ok(vec![4, 5]));

        let result = AddressUtils::function_delegate_call(
            &mut contract,
            BOB,
            &[1, 2, 3],
        );

        assert_eq!(result.unwrap(), vec![4, 5]);
    }

    #[test]
    fn function_delegate_call_errors_when_target_has_no_code() {
        let vm = TestVM::default();
        let mut contract = Caller::from(&vm);
        vm.mock_delegate_call(BOB, vec![1, 2, 3], Ok(vec![]));

        let result = AddressUtils::function_delegate_call(
            &mut contract,
            BOB,
            &[1, 2, 3],
        );

        assert!(matches!(
            result,
            Err(Error::EmptyCode(AddressEmptyCode { target })) if target == BOB
        ));
    }

    #[test]
    fn function_delegate_call_accepts_empty_data_from_a_contract() {
        let vm = TestVM::default();
        let mut contract = Caller::from(&vm);
        vm.set_code(BOB, vec![0x60, 0x80]);
        vm.mock_delegate_call(BOB, vec![1, 2, 3], Ok(vec![]));

        let result = AddressUtils::function_delegate_call(
            &mut contract,
            BOB,
            &[1, 2, 3],
        );

        assert_eq!(result.unwrap(), Vec::<u8>::new());
    }

    #[test]
    fn function_delegate_call_errors_when_call_reverts_without_reason() {
        let vm = TestVM::default();
        let mut contract = Caller::from(&vm);
        vm.mock_delegate_call(BOB, vec![1, 2, 3], Err(vec![]));

        let result = AddressUtils::function_delegate_call(
            &mut contract,
            BOB,
            &[1, 2, 3],
        );

        assert!(matches!(result, Err(Error::FailedCall(_))));
    }

    #[test]
    fn function_delegate_call_bubbles_up_the_revert_reason() {
        let vm = TestVM::default();
        let mut contract = Caller::from(&vm);
        let reason = vec![0xde, 0xad, 0xbe, 0xef];
        vm.mock_delegate_call(BOB, vec![1, 2, 3], Err(reason.clone()));

        let result = AddressUtils::function_delegate_call(
            &mut contract,
            BOB,
            &[1, 2, 3],
        );

        assert!(matches!(
            result,
            Err(Error::FailedCallWithReason(FailedCallWithReason {
                reason: bubbled
            })) if bubbled.to_vec() == reason
        ));
    }
}
