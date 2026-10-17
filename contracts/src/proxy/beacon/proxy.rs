//! This contract implements a proxy that gets the implementation address for
//! each call from an [`UpgradeableBeacon`][UpgradeableBeacon].
//!
//! The beacon address can only be set once during construction, and cannot be
//! changed afterwards. It is stored in the beacon storage slot specified by
//! [ERC-1967], so it cannot overlap with the implementation's storage.
//!
//! CAUTION: Since the beacon address can never be changed, you must ensure that
//! you either control the beacon, or trust the beacon to not upgrade the
//! implementation maliciously.
//!
//! IMPORTANT: Do not use the implementation logic to modify the beacon storage
//! slot. Doing so would leave the proxy in an inconsistent state where the
//! beacon storage slot does not match the beacon address.
//!
//! [UpgradeableBeacon]: super::UpgradeableBeacon
//! [ERC-1967]: https://eips.ethereum.org/EIPS/eip-1967

use alloc::{vec, vec::Vec};

use alloy_primitives::Address;
use stylus_sdk::{abi::Bytes, prelude::*, storage::StorageAddress};

use crate::proxy::{
    beacon::IBeaconInterface,
    erc1967::{Erc1967Utils, Error},
    IProxy,
};

/// State of an [`BeaconProxy`] contract.
#[storage]
pub struct BeaconProxy {
    /// Reserved slot retained for storage compatibility. The active beacon is
    /// read from the ERC-1967 slot instead.
    #[allow(dead_code)]
    beacon: StorageAddress,
}

/// NOTE: Implementation of [`TopLevelStorage`] to be able use `&mut self` when
/// calling other contracts and not `&mut (impl TopLevelStorage +
/// BorrowMut<Self>)`. Should be fixed in the future by the Stylus team.
unsafe impl TopLevelStorage for BeaconProxy {}

impl BeaconProxy {
    /// Initializes the proxy with `beacon`.
    ///
    /// If `data` is nonempty, it's used as data in a delegate call to the
    /// implementation returned by the beacon. This will typically be an
    /// encoded function call, and allows initializing the storage of the proxy
    /// like a Solidity constructor.
    ///
    /// # Arguments
    ///
    /// * `&mut self` - Write access to the contract's state.
    /// * `beacon` - The beacon address.
    /// * `data` - The data to pass to the beacon.
    ///
    /// # Errors
    ///
    /// * [`Error::InvalidBeacon`] - If the beacon is not a contract with the
    ///   interface [`IBeacon`][IBeacon].
    /// * [`Error::NonPayable`] - If the data is empty and the VM call value is
    ///   not [`U256::ZERO`][U256].
    ///
    /// [IBeacon]: super::IBeacon
    /// [U256]: alloy_primitives::U256
    pub fn constructor(
        &mut self,
        beacon: Address,
        data: &Bytes,
    ) -> Result<(), Error> {
        Erc1967Utils::upgrade_beacon_to_and_call(self, beacon, data)?;
        Ok(())
    }

    /// Returns the beacon.
    ///
    /// # Arguments
    ///
    /// * `&self` - Read access to the contract's state.
    #[must_use]
    pub fn get_beacon(&self) -> Address {
        Erc1967Utils::get_beacon(self)
    }
}

unsafe impl IProxy for BeaconProxy {
    fn implementation(&self) -> Result<Address, Vec<u8>> {
        Ok(IBeaconInterface::new(self.get_beacon())
            .implementation(self.vm(), Call::new())?)
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
    struct BeaconProxyExample {
        beacon_proxy: BeaconProxy,
    }

    #[public]
    impl BeaconProxyExample {
        #[constructor]
        fn constructor(
            &mut self,
            beacon: Address,
            data: Bytes,
        ) -> Result<(), Error> {
            self.beacon_proxy.constructor(beacon, &data)
        }

        fn get_beacon(&self) -> Address {
            self.beacon_proxy.get_beacon()
        }

        fn implementation(&self) -> Result<Address, Vec<u8>> {
            self.beacon_proxy.implementation()
        }

        #[fallback]
        fn fallback(&mut self, calldata: &[u8]) -> ArbResult {
            unsafe { self.beacon_proxy.do_fallback(calldata) }
        }
    }

    sol! {
        interface IBeaconMock {
            function implementation() external view returns (address);
        }

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
        let beacon = Address::from([161u8; 20]);
        let implementation = Address::from([162u8; 20]);
        let alice = Address::from([1u8; 20]);
        vm.set_contract_address(proxy_address);
        vm.set_code(beacon, vec![1]);
        vm.set_code(implementation, vec![1]);
        vm.mock_static_call(
            beacon,
            IBeaconMock::implementationCall {}.abi_encode(),
            Ok(implementation.abi_encode()),
        );
        let mut proxy = BeaconProxyExample::from(&vm);
        ({
            vm.set_sender(alice);
            &mut proxy
        })
        .constructor(beacon, vec![].into())
        .unwrap();
        assert_eq!(
            ({
                vm.set_sender(alice);
                &mut proxy
            })
            .implementation()
            .unwrap(),
            implementation
        );
        assert_eq!(
            ({
                vm.set_sender(alice);
                &mut proxy
            })
            .get_beacon(),
            beacon
        );
    }

    #[test]
    fn constructs_with_data() {
        let vm = TestVM::default();
        let proxy_address = Address::from([160u8; 20]);
        let beacon = Address::from([161u8; 20]);
        let implementation = Address::from([162u8; 20]);
        let alice = Address::from([1u8; 20]);
        let amount = U256::from(1000);
        vm.set_contract_address(proxy_address);
        vm.set_code(beacon, vec![1]);
        vm.set_code(implementation, vec![1]);
        vm.mock_static_call(
            beacon,
            IBeaconMock::implementationCall {}.abi_encode(),
            Ok(implementation.abi_encode()),
        );
        let mut proxy = BeaconProxyExample::from(&vm);
        let mint = IERC20::mintCall { to: alice, value: amount }.abi_encode();
        vm.mock_delegate_call(implementation, mint.clone(), Ok(Vec::new()));
        ({
            vm.set_sender(alice);
            &mut proxy
        })
        .constructor(beacon, mint.into())
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
        assert_eq!(
            ({
                vm.set_sender(alice);
                &mut proxy
            })
            .get_beacon(),
            beacon
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
        let beacon = Address::from([161u8; 20]);
        let implementation = Address::from([162u8; 20]);
        let alice = Address::from([1u8; 20]);
        let bob = Address::from([2u8; 20]);
        let amount = U256::from(1000);
        vm.set_contract_address(proxy_address);
        vm.set_code(beacon, vec![1]);
        vm.set_code(implementation, vec![1]);
        vm.mock_static_call(
            beacon,
            IBeaconMock::implementationCall {}.abi_encode(),
            Ok(implementation.abi_encode()),
        );
        let mut proxy = BeaconProxyExample::from(&vm);
        ({
            vm.set_sender(alice);
            &mut proxy
        })
        .constructor(beacon, vec![].into())
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
        let beacon = Address::from([161u8; 20]);
        let implementation = Address::from([162u8; 20]);
        let alice = Address::from([1u8; 20]);
        let bob = Address::from([2u8; 20]);
        let amount = U256::from(1000);
        vm.set_contract_address(proxy_address);
        vm.set_code(beacon, vec![1]);
        vm.set_code(implementation, vec![1]);
        vm.mock_static_call(
            beacon,
            IBeaconMock::implementationCall {}.abi_encode(),
            Ok(implementation.abi_encode()),
        );
        let mut proxy = BeaconProxyExample::from(&vm);
        ({
            vm.set_sender(alice);
            &mut proxy
        })
        .constructor(beacon, vec![].into())
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
