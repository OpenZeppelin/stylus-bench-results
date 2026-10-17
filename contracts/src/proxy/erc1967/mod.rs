//! This contract implements an upgradeable proxy. It is upgradeable because
//! calls are delegated to an implementation address that can be changed. This
//! address is stored in storage in the location specified by
//! [ERC-1967], so that it doesn't conflict with the storage layout of the
//! implementation behind the proxy.
//!
//! [ERC-1967]: https://eips.ethereum.org/EIPS/eip-1967
use alloc::{vec, vec::Vec};

use alloy_primitives::Address;
use stylus_sdk::{abi::Bytes, prelude::*};

use crate::proxy::IProxy;

pub mod utils;

pub use sol::*;
pub use utils::{Erc1967Utils, Error};

#[cfg_attr(coverage_nightly, coverage(off))]
mod sol {
    use alloy_sol_macro::sol;

    sol! {
        /// Emitted when the implementation is upgraded.
        #[derive(Debug)]
        #[allow(missing_docs)]
        event Upgraded(address indexed implementation);

        /// Emitted when the admin account has changed.
        #[derive(Debug)]
        #[allow(missing_docs)]
        event AdminChanged(address previous_admin, address new_admin);

        /// Emitted when the beacon is changed.
        #[derive(Debug)]
        #[allow(missing_docs)]
        event BeaconUpgraded(address indexed beacon);
    }
}

/// State of an [`Erc1967Proxy`] token.
#[storage]
pub struct Erc1967Proxy;

/// NOTE: Implementation of [`TopLevelStorage`] to be able use `&mut self` when
/// calling other contracts and not `&mut (impl TopLevelStorage +
/// BorrowMut<Self>)`. Should be fixed in the future by the Stylus team.
unsafe impl TopLevelStorage for Erc1967Proxy {}

impl Erc1967Proxy {
    /// Initializes the upgradeable proxy with an initial implementation
    /// specified by `implementation`.
    ///
    /// If `data` is nonempty, it's used as data in a delegate call to
    /// `implementation`. This will typically be an encoded function call,
    /// and allows initializing the storage of the proxy like a Solidity
    /// constructor.
    ///
    /// # Arguments
    ///
    /// * `&mut self` - Write access to the contract's state.
    /// * `implementation` - Address of the implementation contract.
    /// * `data` - Data to pass to the implementation contract.
    ///
    /// # Errors
    ///
    /// * [`Error::InvalidImplementation`] - If the implementation is not a
    ///   valid implementation.
    /// * [`Error::NonPayable`] - If `data` is empty and the VM call value is
    ///   not [`U256::ZERO`][U256].
    /// * [`Error::FailedCall`] - If the call to the implementation fails.
    /// * [`Error::FailedCallWithReason`] - If the call to the implementation
    ///   fails with a revert reason.
    ///
    /// [U256]: alloy_primitives::U256
    pub fn constructor(
        &mut self,
        implementation: Address,
        data: &Bytes,
    ) -> Result<(), Error> {
        Erc1967Utils::upgrade_to_and_call(self, implementation, data)
    }
}

unsafe impl IProxy for Erc1967Proxy {
    fn implementation(&self) -> Result<Address, Vec<u8>> {
        Ok(Erc1967Utils::get_implementation(self))
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::needless_pass_by_value)]

    use alloy_sol_macro::sol;
    use alloy_sol_types::{SolCall, SolError, SolValue};
    use stylus_sdk::testing::TestVM;
    use stylus_sdk::{
        alloy_primitives::{Address, U256},
        prelude::*,
        ArbResult,
    };

    use super::*;
    use crate::token::erc20;

    #[entrypoint]
    #[storage]
    struct Erc1967ProxyExample {
        erc1967: Erc1967Proxy,
    }

    #[public]
    impl Erc1967ProxyExample {
        #[constructor]
        fn constructor(
            &mut self,
            implementation: Address,
            data: Bytes,
        ) -> Result<(), Error> {
            self.erc1967.constructor(implementation, &data)
        }

        fn implementation(&self) -> Result<Address, Vec<u8>> {
            self.erc1967.implementation()
        }

        #[fallback]
        fn fallback(&mut self, calldata: &[u8]) -> ArbResult {
            unsafe { self.erc1967.do_fallback(calldata) }
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
        vm.set_contract_address(proxy_address);
        vm.set_code(implementation, vec![1]);
        let mut proxy = Erc1967ProxyExample::from(&vm);
        ({
            vm.set_sender(Address::from([1u8; 20]));
            &mut proxy
        })
        .constructor(implementation, vec![].into())
        .expect("construct");
        assert_eq!(proxy.implementation().unwrap(), implementation);
    }

    #[test]
    fn constructs_with_data() {
        let vm = TestVM::default();
        let proxy_address = Address::from([160u8; 20]);
        let implementation = Address::from([161u8; 20]);
        let alice = Address::from([1u8; 20]);
        let amount = U256::from(1000);
        vm.set_contract_address(proxy_address);
        vm.set_code(implementation, vec![1]);
        let mut proxy = Erc1967ProxyExample::from(&vm);
        let mint = IERC20::mintCall { to: alice, value: amount }.abi_encode();
        vm.mock_delegate_call(implementation, mint.clone(), Ok(Vec::new()));
        ({
            vm.set_sender(alice);
            &mut proxy
        })
        .constructor(implementation, mint.into())
        .expect("construct with init data");
        assert_eq!(
            ({
                vm.set_sender(alice);
                &mut proxy
            })
            .implementation()
            .unwrap(),
            implementation
        );
        let balance = IERC20::balanceOfCall { account: alice }.abi_encode();
        vm.mock_delegate_call(
            implementation,
            balance.clone(),
            Ok(amount.abi_encode()),
        );
        assert_eq!(
            ({
                vm.set_sender(alice);
                &mut proxy
            })
            .fallback(&balance)
            .unwrap(),
            amount.abi_encode()
        );
        let supply = IERC20::totalSupplyCall {}.abi_encode();
        vm.mock_delegate_call(
            implementation,
            supply.clone(),
            Ok(amount.abi_encode()),
        );
        assert_eq!(
            ({
                vm.set_sender(alice);
                &mut proxy
            })
            .fallback(&supply)
            .unwrap(),
            amount.abi_encode()
        );
    }

    #[test]
    fn fallback() {
        let vm = TestVM::default();
        let proxy_address = Address::from([160u8; 20]);
        let implementation = Address::from([161u8; 20]);
        let alice = Address::from([1u8; 20]);
        let bob = Address::from([2u8; 20]);
        let amount = U256::from(1000);
        vm.set_contract_address(proxy_address);
        vm.set_code(implementation, vec![1]);
        let mut proxy = Erc1967ProxyExample::from(&vm);
        ({
            vm.set_sender(alice);
            &mut proxy
        })
        .constructor(implementation, vec![].into())
        .expect("construct");
        let balance_alice =
            IERC20::balanceOfCall { account: alice }.abi_encode();
        let supply = IERC20::totalSupplyCall {}.abi_encode();
        let mint = IERC20::mintCall { to: alice, value: amount }.abi_encode();
        let transfer =
            IERC20::transferCall { to: bob, value: amount }.abi_encode();
        let balance_bob = IERC20::balanceOfCall { account: bob }.abi_encode();
        for (call, output) in [
            (balance_alice.clone(), U256::ZERO.abi_encode()),
            (supply.clone(), U256::ZERO.abi_encode()),
            (mint, Vec::new()),
            (balance_alice.clone(), amount.abi_encode()),
            (supply.clone(), amount.abi_encode()),
            (transfer, Vec::new()),
            (balance_alice, U256::ZERO.abi_encode()),
            (balance_bob, amount.abi_encode()),
            (supply, amount.abi_encode()),
        ] {
            vm.mock_delegate_call(
                implementation,
                call.clone(),
                Ok(output.clone()),
            );
            assert_eq!(
                ({
                    vm.set_sender(alice);
                    &mut proxy
                })
                .fallback(&call)
                .unwrap(),
                output
            );
        }
    }

    #[test]
    fn fallback_returns_error() {
        let vm = TestVM::default();
        let proxy_address = Address::from([160u8; 20]);
        let implementation = Address::from([161u8; 20]);
        let alice = Address::from([1u8; 20]);
        let bob = Address::from([2u8; 20]);
        let amount = U256::from(1000);
        vm.set_contract_address(proxy_address);
        vm.set_code(implementation, vec![1]);
        let mut proxy = Erc1967ProxyExample::from(&vm);
        ({
            vm.set_sender(alice);
            &mut proxy
        })
        .constructor(implementation, vec![].into())
        .expect("construct");
        let call = IERC20::transferCall { to: bob, value: amount }.abi_encode();
        let revert = erc20::ERC20InsufficientBalance {
            sender: alice,
            balance: U256::ZERO,
            needed: amount,
        }
        .abi_encode();
        vm.mock_delegate_call(
            implementation,
            call.clone(),
            Err(revert.clone()),
        );
        assert_eq!(
            ({
                vm.set_sender(alice);
                &mut proxy
            })
            .fallback(&call)
            .unwrap_err(),
            revert
        );
    }
}
