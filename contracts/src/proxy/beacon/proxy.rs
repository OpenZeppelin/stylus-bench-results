//! This contract implements a proxy that gets the implementation address for
//! each call from an [`UpgradeableBeacon`][UpgradeableBeacon].
//!
//! The beacon address is stored in the beacon storage slot specified by
//! [ERC-1967], so it can be accessed externally and is not vulnerable to
//! collisions with implementation storage.
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
    /// Reserved to preserve the storage layout of v0.3.0.
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
    /// * [`Error::NonPayable`] - If the data is empty and transaction value is
    ///   nonzero.
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

    use crate::test_support::*;
    use alloy_sol_macro::sol;
    use alloy_sol_types::{SolCall, SolError, SolValue};
    use stylus_sdk::{
        alloy_primitives::{Address, U256},
        function_selector,
        prelude::*,
        testing::TestVM,
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

    impl BeaconProxyExample {
        fn overwrite_beacon_slot(&mut self, beacon: Address) {
            self.beacon_proxy.beacon.set(beacon);
        }
    }

    // The SDK TestVM provides external-call responses explicitly, so these
    // storage fixtures only identify addresses with code.
    #[storage]
    struct Beacon;

    unsafe impl TopLevelStorage for Beacon {}

    #[storage]
    struct Erc20Example;

    unsafe impl TopLevelStorage for Erc20Example {}

    sol! {
        interface IERC20 {
            function balanceOf(address account) external view returns (uint256);
            function totalSupply() external view returns (uint256);
            function mint(address to, uint256 value) external;
            function transfer(address to, uint256 value) external returns (bool);
        }
    }

    fn mock_beacon_implementation(
        vm: &TestVM,
        beacon: Address,
        implementation: Address,
    ) {
        vm.mock_static_call(
            beacon,
            function_selector!("implementation").to_vec(),
            Ok(implementation.abi_encode()),
        );
    }

    #[openzeppelin_stylus_proc::vm_test]
    fn constructs(
        proxy: Contract<BeaconProxyExample>,
        beacon: Contract<Beacon>,
        erc20: Contract<Erc20Example>,
        alice: Address,
    ) {
        mock_beacon_implementation(
            &proxy.vm(),
            beacon.address(),
            erc20.address(),
        );

        proxy
            .sender(alice)
            .constructor(beacon.address(), vec![].into())
            .unwrap();

        let implementation = proxy
            .sender(alice)
            .implementation()
            .expect("should be able to get implementation");
        assert_eq!(implementation, erc20.address());

        let beacon_address = proxy.sender(alice).get_beacon();
        assert_eq!(beacon_address, beacon.address());
    }

    #[openzeppelin_stylus_proc::vm_test]
    fn implementation_lookup_ignores_colliding_proxy_slot(
        proxy: Contract<BeaconProxyExample>,
        beacon: Contract<Beacon>,
        another_beacon: Contract<Beacon>,
        erc20: Contract<Erc20Example>,
        another_erc20: Contract<Erc20Example>,
        alice: Address,
    ) {
        mock_beacon_implementation(
            &proxy.vm(),
            beacon.address(),
            erc20.address(),
        );
        mock_beacon_implementation(
            &proxy.vm(),
            another_beacon.address(),
            another_erc20.address(),
        );
        proxy
            .sender(alice)
            .constructor(beacon.address(), vec![].into())
            .expect("should be able to construct");

        // Delegate-called implementation logic can write this ordinary slot.
        proxy.sender(alice).overwrite_beacon_slot(another_beacon.address());

        assert_eq!(proxy.sender(alice).implementation(), Ok(erc20.address()),);
    }

    #[openzeppelin_stylus_proc::vm_test]
    fn constructs_with_data(
        proxy: Contract<BeaconProxyExample>,
        beacon: Contract<Beacon>,
        erc20: Contract<Erc20Example>,
        alice: Address,
    ) {
        mock_beacon_implementation(
            &proxy.vm(),
            beacon.address(),
            erc20.address(),
        );

        let amount = U256::from(1000);

        let data = IERC20::mintCall { to: alice, value: amount }.abi_encode();
        proxy.vm().mock_delegate_call(
            erc20.address(),
            data.clone(),
            Ok(vec![]),
        );

        proxy
            .sender(alice)
            .constructor(beacon.address(), data.into())
            .expect("should be able to construct");

        let implementation = proxy
            .sender(alice)
            .implementation()
            .expect("should be able to get implementation");
        assert_eq!(implementation, erc20.address());

        let beacon_address = proxy.sender(alice).get_beacon();
        assert_eq!(beacon_address, beacon.address());

        let balance_of_alice_call =
            IERC20::balanceOfCall { account: alice }.abi_encode();
        proxy.vm().mock_delegate_call(
            erc20.address(),
            balance_of_alice_call.clone(),
            Ok(amount.abi_encode()),
        );
        let balance = proxy
            .sender(alice)
            .fallback(&balance_of_alice_call)
            .expect("should be able to get balance");
        assert_eq!(balance, amount.abi_encode());

        let total_supply_call = IERC20::totalSupplyCall {}.abi_encode();
        proxy.vm().mock_delegate_call(
            erc20.address(),
            total_supply_call.clone(),
            Ok(amount.abi_encode()),
        );
        let total_supply = proxy
            .sender(alice)
            .fallback(&total_supply_call)
            .expect("should be able to get total supply");
        assert_eq!(total_supply, amount.abi_encode());
    }

    #[openzeppelin_stylus_proc::vm_test]
    fn fallback(
        proxy: Contract<BeaconProxyExample>,
        beacon: Contract<Beacon>,
        erc20: Contract<Erc20Example>,
        alice: Address,
        bob: Address,
    ) {
        mock_beacon_implementation(
            &proxy.vm(),
            beacon.address(),
            erc20.address(),
        );

        proxy
            .sender(alice)
            .constructor(beacon.address(), vec![].into())
            .expect("should be able to construct");

        // verify initial balance is [`U256::ZERO`].
        let balance_of_alice_call =
            IERC20::balanceOfCall { account: alice }.abi_encode();
        proxy.vm().mock_delegate_call(
            erc20.address(),
            balance_of_alice_call.clone(),
            Ok(U256::ZERO.abi_encode()),
        );
        let balance = proxy
            .sender(alice)
            .fallback(&balance_of_alice_call)
            .expect("should be able to get balance");
        assert_eq!(balance, U256::ZERO.abi_encode());

        let total_supply_call = IERC20::totalSupplyCall {}.abi_encode();
        proxy.vm().mock_delegate_call(
            erc20.address(),
            total_supply_call.clone(),
            Ok(U256::ZERO.abi_encode()),
        );
        let total_supply = proxy
            .sender(alice)
            .fallback(&total_supply_call)
            .expect("should be able to get total supply");
        assert_eq!(total_supply, U256::ZERO.abi_encode());

        // mint 1000 tokens.
        let amount = U256::from(1000);

        let mint_call =
            IERC20::mintCall { to: alice, value: amount }.abi_encode();
        proxy.vm().mock_delegate_call(
            erc20.address(),
            mint_call.clone(),
            Ok(vec![]),
        );
        proxy
            .sender(alice)
            .fallback(&mint_call)
            .expect("should be able to mint");
        // check that the balance can be accurately fetched through the proxy.
        proxy.vm().mock_delegate_call(
            erc20.address(),
            balance_of_alice_call.clone(),
            Ok(amount.abi_encode()),
        );
        proxy.vm().mock_delegate_call(
            erc20.address(),
            total_supply_call.clone(),
            Ok(amount.abi_encode()),
        );
        let balance = proxy
            .sender(alice)
            .fallback(&balance_of_alice_call)
            .expect("should be able to get balance");
        assert_eq!(balance, amount.abi_encode());

        let total_supply = proxy
            .sender(alice)
            .fallback(&total_supply_call)
            .expect("should be able to get total supply");
        assert_eq!(total_supply, amount.abi_encode());

        // check that the balance can be transferred through the proxy.
        let transfer_call =
            IERC20::transferCall { to: bob, value: amount }.abi_encode();
        proxy.vm().mock_delegate_call(
            erc20.address(),
            transfer_call.clone(),
            Ok(true.abi_encode()),
        );
        let transfer_result = proxy
            .sender(alice)
            .fallback(&transfer_call)
            .expect("should be able to transfer");
        assert_eq!(transfer_result, true.abi_encode());

        proxy.vm().mock_delegate_call(
            erc20.address(),
            balance_of_alice_call.clone(),
            Ok(U256::ZERO.abi_encode()),
        );
        let balance = proxy
            .sender(alice)
            .fallback(&balance_of_alice_call)
            .expect("should be able to get balance");
        assert_eq!(balance, U256::ZERO.abi_encode());

        let balance_of_bob_call =
            IERC20::balanceOfCall { account: bob }.abi_encode();
        proxy.vm().mock_delegate_call(
            erc20.address(),
            balance_of_bob_call.clone(),
            Ok(amount.abi_encode()),
        );
        let balance = proxy
            .sender(alice)
            .fallback(&balance_of_bob_call)
            .expect("should be able to get balance");
        assert_eq!(balance, amount.abi_encode());

        let total_supply = proxy
            .sender(alice)
            .fallback(&total_supply_call)
            .expect("should be able to get total supply");
        assert_eq!(total_supply, amount.abi_encode());
    }

    #[openzeppelin_stylus_proc::vm_test]
    fn fallback_returns_error(
        proxy: Contract<BeaconProxyExample>,
        beacon: Contract<Beacon>,
        erc20: Contract<Erc20Example>,
        alice: Address,
        bob: Address,
    ) {
        mock_beacon_implementation(
            &proxy.vm(),
            beacon.address(),
            erc20.address(),
        );

        proxy
            .sender(alice)
            .constructor(beacon.address(), vec![].into())
            .expect("should be able to construct");

        let amount = U256::from(1000);
        let transfer_call =
            IERC20::transferCall { to: bob, value: amount }.abi_encode();
        proxy.vm().mock_delegate_call(
            erc20.address(),
            transfer_call.clone(),
            Err(erc20::ERC20InsufficientBalance {
                sender: alice,
                balance: U256::ZERO,
                needed: amount,
            }
            .abi_encode()),
        );
        let err = proxy
            .sender(alice)
            .fallback(&transfer_call)
            .expect_error("should revert on transfer");

        assert_eq!(
            err,
            erc20::ERC20InsufficientBalance {
                sender: alice,
                balance: U256::ZERO,
                needed: amount,
            }
            .abi_encode()
        );
    }
}
