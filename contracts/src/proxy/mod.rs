//! This is a low-level set of contracts implementing different proxy patterns
//! with and without upgradeability.
use alloc::vec::Vec;

use alloy_primitives::Address;
use stylus_sdk::{
    call::{self},
    prelude::*,
    stylus_core::calls::errors::Error,
};

pub mod beacon;
pub mod erc1967;
pub mod utils;

/// This trait provides a fallback function that delegates all calls to another
/// contract using the Stylus `delegate_call` function. We
/// refer to the second contract as the _implementation_ behind the proxy, and
/// it has to be specified by overriding the virtual [`IProxy::implementation`]
/// function.
///
/// Additionally, delegation to the implementation can be triggered manually
/// through the [`IProxy::do_fallback`] function, or to a different contract
/// through the [`IProxy::delegate`] function.
///
/// The success and return data of the delegated call will be returned back
/// to the caller of the proxy.
///
/// # Safety
///
/// This trait is unsafe to implement because it uses the `unsafe`
/// `delegate_call` function.
///
/// The caller must ensure that `self` is a valid contract storage context.
///
/// The caller must ensure that the implementation contract is a valid contract
/// address.
///
pub unsafe trait IProxy: TopLevelStorage + HostAccess + Sized {
    /// Delegates the current call to [`IProxy::implementation`].
    ///
    /// This function does not return to its internal call site, it will
    /// return directly to the external caller.
    ///
    /// # Arguments
    ///
    /// * `&mut self` - Write access to the contract's state.
    /// * `implementation` - The address of the implementation contract.
    /// * `calldata` - The calldata to delegate to the implementation contract.
    ///
    /// # Errors
    ///
    /// Returns a [`stylus_sdk::stylus_core::calls::errors::Error`] if the delegate call fails. This
    /// error may represent:
    /// * A revert from the implementation containing the revert data.
    /// * Failure to decode the return data from the implementation contract.
    /// * Other low-level call failures as defined by the Stylus SDK.
    #[allow(clippy::missing_safety_doc)]
    unsafe fn delegate(
        &mut self,
        implementation: Address,
        calldata: &[u8],
    ) -> Result<Vec<u8>, Error> {
        let call = Call::new_mutating(self);
        call::delegate_call(self.vm(), call, implementation, calldata)
    }

    /// This is a virtual function that should be overridden so it
    /// returns the address to which the fallback function and
    /// [`IProxy::do_fallback`] should delegate.
    ///
    /// # Arguments
    ///
    /// * `&self` - Read access to the contract's state.
    ///
    /// # Errors
    ///
    /// Implementing contracts should define their own error types for this
    /// function. Typically, errors may include:
    /// * The implementation address is invalid (e.g., not a contract).
    /// * The implementation is not a contract.
    ///
    /// The error should be encoded as a `Vec<u8>`.
    fn implementation(&self) -> Result<Address, Vec<u8>>;

    /// Fallback function that delegates calls to the address returned
    /// by [`IProxy::implementation`]. Will run if no other function in the
    /// contract matches the call data.
    ///
    /// # Arguments
    ///
    /// * `&mut self` - Write access to the contract's state.
    /// * `calldata` - The calldata to delegate to the implementation contract.
    ///
    /// # Errors
    ///
    /// Implementing contracts should define their own error types for this
    /// function. Typically, errors may include:
    /// * The implementation address is invalid (e.g., not a contract).
    /// * The implementation is not a contract.
    /// * The implementation reverted with a reason.
    ///
    /// The error should be encoded as a `Vec<u8>`.
    #[allow(clippy::missing_safety_doc)]
    unsafe fn do_fallback(
        &mut self,
        calldata: &[u8],
    ) -> Result<Vec<u8>, Vec<u8>> {
        Ok(self.delegate(self.implementation()?, calldata)?)
    }
}

#[cfg(test)]
mod tests {
    use alloy_sol_macro::sol;
    use alloy_sol_types::{SolCall, SolError, SolValue};
    use stylus_sdk::testing::TestVM;
    use stylus_sdk::{
        alloy_primitives::{Address, U256},
        prelude::*,
        storage::{StorageAddress, StorageBool},
        ArbResult,
    };

    use super::*;
    use crate::token::erc20;

    #[entrypoint]
    #[storage]
    struct ProxyExample {
        implementation: StorageAddress,
        error_on_implementation: StorageBool,
    }

    #[public]
    impl ProxyExample {
        #[constructor]
        fn constructor(&mut self, implementation: Address) {
            self.implementation.set(implementation);
        }

        fn set_error_on_implementation(
            &mut self,
            error_on_implementation: bool,
        ) {
            self.error_on_implementation.set(error_on_implementation);
        }

        fn implementation(&self) -> Result<Address, Vec<u8>> {
            if self.error_on_implementation.get() {
                return Err("implementation error".abi_encode());
            }
            Ok(self.implementation.get())
        }

        #[fallback]
        fn fallback(&mut self, calldata: &[u8]) -> ArbResult {
            unsafe { self.do_fallback(calldata) }
        }
    }

    unsafe impl IProxy for ProxyExample {
        fn implementation(&self) -> Result<Address, Vec<u8>> {
            Self::implementation(self)
        }
    }

    sol! {
        interface IERC20 {
            function balanceOf(address account) external view returns (uint256);
            function totalSupply() external view returns (uint256);
            function mint(address to, uint256 value) external;
            function transfer(address to, uint256 value) external returns (bool);
        }
    }

    #[test]
    fn constructs() {
        let vm = TestVM::default();
        let proxy_address = Address::from([160u8; 20]);
        let implementation = Address::from([161u8; 20]);
        let alice = Address::from([1u8; 20]);
        vm.set_contract_address(proxy_address);
        let mut proxy = ProxyExample::from(&vm);
        ({
            vm.set_sender(alice);
            &mut proxy
        })
        .constructor(implementation);
        assert_eq!(
            ({
                vm.set_sender(alice);
                &mut proxy
            })
            .implementation()
            .unwrap(),
            implementation
        );
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn delegate() {
        let vm = TestVM::default();
        let proxy_address = Address::from([160u8; 20]);
        let implementation = Address::from([161u8; 20]);
        let alice = Address::from([1u8; 20]);
        let bob = Address::from([2u8; 20]);
        let amount = U256::from(1000);
        vm.set_contract_address(proxy_address);
        vm.set_code(implementation, vec![1]);
        let mut proxy = ProxyExample::from(&vm);
        ({
            vm.set_sender(alice);
            &mut proxy
        })
        .constructor(implementation);

        let balance_alice =
            IERC20::balanceOfCall { account: alice }.abi_encode();
        let total_supply = IERC20::totalSupplyCall {}.abi_encode();
        let mint = IERC20::mintCall { to: alice, value: amount }.abi_encode();
        let transfer =
            IERC20::transferCall { to: bob, value: amount }.abi_encode();
        let balance_bob = IERC20::balanceOfCall { account: bob }.abi_encode();

        vm.mock_delegate_call(
            implementation,
            balance_alice.clone(),
            Ok(U256::ZERO.abi_encode()),
        );
        assert_eq!(
            ({
                vm.set_sender(alice);
                &mut proxy
            })
            .fallback(&balance_alice)
            .unwrap(),
            U256::ZERO.abi_encode()
        );
        vm.mock_delegate_call(
            implementation,
            total_supply.clone(),
            Ok(U256::ZERO.abi_encode()),
        );
        assert_eq!(
            ({
                vm.set_sender(alice);
                &mut proxy
            })
            .fallback(&total_supply)
            .unwrap(),
            U256::ZERO.abi_encode()
        );
        vm.mock_delegate_call(implementation, mint, Ok(Vec::new()));
        assert_eq!(
            ({
                vm.set_sender(alice);
                &mut proxy
            })
            .fallback(
                &IERC20::mintCall { to: alice, value: amount }.abi_encode()
            )
            .unwrap(),
            Vec::<u8>::new()
        );
        vm.mock_delegate_call(
            implementation,
            balance_alice.clone(),
            Ok(amount.abi_encode()),
        );
        assert_eq!(
            ({
                vm.set_sender(alice);
                &mut proxy
            })
            .fallback(&balance_alice)
            .unwrap(),
            amount.abi_encode()
        );
        vm.mock_delegate_call(
            implementation,
            total_supply.clone(),
            Ok(amount.abi_encode()),
        );
        assert_eq!(
            ({
                vm.set_sender(alice);
                &mut proxy
            })
            .fallback(&total_supply)
            .unwrap(),
            amount.abi_encode()
        );
        vm.mock_delegate_call(implementation, transfer.clone(), Ok(Vec::new()));
        assert_eq!(
            ({
                vm.set_sender(alice);
                &mut proxy
            })
            .fallback(&transfer)
            .unwrap(),
            Vec::<u8>::new()
        );
        vm.mock_delegate_call(
            implementation,
            balance_alice.clone(),
            Ok(U256::ZERO.abi_encode()),
        );
        assert_eq!(
            ({
                vm.set_sender(alice);
                &mut proxy
            })
            .fallback(&balance_alice)
            .unwrap(),
            U256::ZERO.abi_encode()
        );
        vm.mock_delegate_call(
            implementation,
            balance_bob.clone(),
            Ok(amount.abi_encode()),
        );
        assert_eq!(
            ({
                vm.set_sender(alice);
                &mut proxy
            })
            .fallback(&balance_bob)
            .unwrap(),
            amount.abi_encode()
        );
        vm.mock_delegate_call(
            implementation,
            total_supply.clone(),
            Ok(amount.abi_encode()),
        );
        assert_eq!(
            ({
                vm.set_sender(alice);
                &mut proxy
            })
            .fallback(&total_supply)
            .unwrap(),
            amount.abi_encode()
        );
    }

    #[test]
    fn delegate_returns_error() {
        let vm = TestVM::default();
        let proxy_address = Address::from([160u8; 20]);
        let implementation = Address::from([161u8; 20]);
        let alice = Address::from([1u8; 20]);
        let bob = Address::from([2u8; 20]);
        let amount = U256::from(1000);
        vm.set_contract_address(proxy_address);
        vm.set_code(implementation, vec![1]);
        let mut proxy = ProxyExample::from(&vm);
        ({
            vm.set_sender(alice);
            &mut proxy
        })
        .constructor(implementation);
        let calldata =
            IERC20::transferCall { to: bob, value: amount }.abi_encode();
        let revert = erc20::ERC20InsufficientBalance {
            sender: alice,
            balance: U256::ZERO,
            needed: amount,
        }
        .abi_encode();
        vm.mock_delegate_call(
            implementation,
            calldata.clone(),
            Err(revert.clone()),
        );
        let err = ({
            vm.set_sender(alice);
            &mut proxy
        })
        .fallback(&calldata)
        .unwrap_err();
        assert_eq!(err, revert);
    }

    #[test]
    fn direct_delegate_to_different_implementation() {
        let vm = TestVM::default();
        let proxy_address = Address::from([160u8; 20]);
        let implementation = Address::from([161u8; 20]);
        let other_implementation = Address::from([162u8; 20]);
        let alice = Address::from([1u8; 20]);
        let amount = U256::from(500);
        vm.set_contract_address(proxy_address);
        vm.set_code(implementation, vec![1]);
        vm.set_code(other_implementation, vec![1]);
        let mut proxy = ProxyExample::from(&vm);
        ({
            vm.set_sender(alice);
            &mut proxy
        })
        .constructor(implementation);
        let calldata = IERC20::balanceOfCall { account: alice }.abi_encode();
        vm.mock_delegate_call(
            other_implementation,
            calldata.clone(),
            Ok(amount.abi_encode()),
        );
        let balance = unsafe {
            ({
                vm.set_sender(alice);
                &mut proxy
            })
            .delegate(other_implementation, &calldata)
            .unwrap()
        };
        assert_eq!(balance, amount.abi_encode());
    }

    #[test]
    fn fallback_reverts_on_implementation_error() {
        let proxy_vm = TestVM::default();
        let proxy_address = Address::from([160u8; 20]);
        proxy_vm.set_contract_address(proxy_address);
        let mut proxy = ProxyExample::from(&proxy_vm);
        let alice = Address::from([1u8; 20]);

        // create proxy with [`Address::ZERO`] as implementation (invalid).
        ({
            proxy_vm.set_sender(alice);
            &mut proxy
        })
        .set_error_on_implementation(true);

        let balance_of_call =
            IERC20::balanceOfCall { account: alice }.abi_encode();
        let err = ({
            proxy_vm.set_sender(alice);
            &mut proxy
        })
        .fallback(&balance_of_call)
        .expect_err("should fail when implementation is zero address");

        assert_eq!(err, "implementation error".abi_encode());
    }
}
