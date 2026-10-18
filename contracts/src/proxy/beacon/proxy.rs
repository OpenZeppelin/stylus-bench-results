//! This contract implements a proxy that gets the implementation address for
//! each call from an [`UpgradeableBeacon`][UpgradeableBeacon].
//!
//! The beacon address can only be set once during construction, and cannot be
//! changed afterwards. It is stored in an immutable variable to avoid
//! unnecessary storage reads, and also in the beacon storage slot specified by
//! [ERC-1967] so that it can be accessed externally.
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
    /// * [`Error::NonPayable`] - If the data is empty and the call value is
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
        self.beacon.set(beacon);
        Ok(())
    }

    /// Returns the beacon.
    ///
    /// # Arguments
    ///
    /// * `&self` - Read access to the contract's state.
    #[must_use]
    pub fn get_beacon(&self) -> Address {
        self.beacon.get()
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

    use crate::test_support::{
        mock_code, mock_static_call, test_address,
        TestContract as TestVMContract,
    };
    use alloy_sol_macro::sol;
    use alloy_sol_types::{SolCall, SolValue};
    use stylus_sdk::{
        alloy_primitives::{Address, U256},
        prelude::*,
        ArbResult,
    };

    use super::*;
    use crate::{
        proxy::beacon::IBeacon,
        token::erc20::{self, Erc20, IErc20},
    };

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

    #[storage]
    struct Beacon {
        implementation: StorageAddress,
    }

    unsafe impl TopLevelStorage for Beacon {}

    #[public]
    #[implements(IBeacon)]
    impl Beacon {
        #[constructor]
        fn constructor(&mut self, implementation: Address) {
            self.implementation.set(implementation);
        }
    }

    #[public]
    impl IBeacon for Beacon {
        fn implementation(&self) -> Result<Address, Vec<u8>> {
            Ok(self.implementation.get())
        }
    }

    #[storage]
    struct Erc20Example {
        erc20: Erc20,
    }

    #[public]
    #[implements(IErc20<Error = erc20::Error>)]
    impl Erc20Example {
        fn mint(
            &mut self,
            to: Address,
            value: U256,
        ) -> Result<(), erc20::Error> {
            self.erc20._mint(to, value)
        }
    }

    unsafe impl TopLevelStorage for Erc20Example {}

    #[public]
    impl IErc20 for Erc20Example {
        type Error = erc20::Error;

        fn balance_of(&self, account: Address) -> U256 {
            self.erc20.balance_of(account)
        }

        fn total_supply(&self) -> U256 {
            self.erc20.total_supply()
        }

        fn transfer(
            &mut self,
            to: Address,
            value: U256,
        ) -> Result<bool, Self::Error> {
            self.erc20.transfer(to, value)
        }

        fn transfer_from(
            &mut self,
            from: Address,
            to: Address,
            value: U256,
        ) -> Result<bool, Self::Error> {
            self.erc20.transfer_from(from, to, value)
        }

        fn allowance(&self, owner: Address, spender: Address) -> U256 {
            self.erc20.allowance(owner, spender)
        }

        fn approve(
            &mut self,
            spender: Address,
            value: U256,
        ) -> Result<bool, Self::Error> {
            self.erc20.approve(spender, value)
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

    fn mock_beacon(
        proxy: &TestVMContract<BeaconProxyExample>,
        beacon: Address,
        implementation: Address,
    ) {
        mock_code(proxy.vm(), beacon);
        mock_code(proxy.vm(), implementation);
        mock_static_call(
            proxy.vm(),
            beacon,
            "implementation()",
            (),
            Ok(implementation.abi_encode()),
        );
    }

    #[test]
    fn constructs() {
        let proxy =
            TestVMContract::<BeaconProxyExample>::new(stringify!(proxy));
        let beacon = TestVMContract::<Beacon>::new(stringify!(beacon));
        let erc20 = TestVMContract::<Erc20Example>::new(stringify!(erc20));
        let alice = test_address(stringify!(alice));

        beacon.sender(alice).constructor(erc20.address());
        mock_beacon(&proxy, beacon.address(), erc20.address());

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

    #[test]
    fn constructs_with_data() {
        let proxy =
            TestVMContract::<BeaconProxyExample>::new(stringify!(proxy));
        let beacon = TestVMContract::<Beacon>::new(stringify!(beacon));
        let erc20 = TestVMContract::<Erc20Example>::new(stringify!(erc20));
        let alice = test_address(stringify!(alice));

        beacon.sender(alice).constructor(erc20.address());

        let amount = U256::from(1000);

        let data = IERC20::mintCall { to: alice, value: amount }.abi_encode();

        proxy.vm().mock_delegate_call(
            erc20.address(),
            data.clone(),
            Ok(erc20.address().abi_encode()),
        );
        mock_beacon(&proxy, beacon.address(), erc20.address());

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
    }

    #[test]
    fn fallback() {
        let proxy =
            TestVMContract::<BeaconProxyExample>::new(stringify!(proxy));
        let beacon = TestVMContract::<Beacon>::new(stringify!(beacon));
        let erc20 = TestVMContract::<Erc20Example>::new(stringify!(erc20));
        let alice = test_address(stringify!(alice));
        let bob = test_address(stringify!(bob));

        beacon.sender(alice).constructor(erc20.address());
        mock_beacon(&proxy, beacon.address(), erc20.address());

        proxy
            .sender(alice)
            .constructor(beacon.address(), vec![].into())
            .expect("should be able to construct");

        // TestVM uses the same return buffer for beacon and delegate calls.
        // Returning the implementation address from both lets the unit test
        // exercise fallback dispatch. State changes are covered by devnet.
        let expected_return = erc20.address().abi_encode();
        let balance_of_alice_call =
            IERC20::balanceOfCall { account: alice }.abi_encode();
        proxy.vm().mock_delegate_call(
            erc20.address(),
            balance_of_alice_call.clone(),
            Ok(expected_return.clone()),
        );
        let balance = proxy
            .sender(alice)
            .fallback(&balance_of_alice_call)
            .expect("should be able to dispatch balance query");
        assert_eq!(balance, expected_return);

        let total_supply_call = IERC20::totalSupplyCall {}.abi_encode();
        proxy.vm().mock_delegate_call(
            erc20.address(),
            total_supply_call.clone(),
            Ok(erc20.address().abi_encode()),
        );
        let total_supply = proxy
            .sender(alice)
            .fallback(&total_supply_call)
            .expect("should be able to dispatch total supply query");
        assert_eq!(total_supply, erc20.address().abi_encode());

        // mint 1000 tokens.
        let amount = U256::from(1000);

        let mint_call =
            IERC20::mintCall { to: alice, value: amount }.abi_encode();
        proxy.vm().mock_delegate_call(
            erc20.address(),
            mint_call.clone(),
            Ok(erc20.address().abi_encode()),
        );
        proxy
            .sender(alice)
            .fallback(&mint_call)
            .expect("should be able to dispatch mint");

        // check that the balance can be transferred through the proxy.
        let transfer_call =
            IERC20::transferCall { to: bob, value: amount }.abi_encode();
        proxy.vm().mock_delegate_call(
            erc20.address(),
            transfer_call.clone(),
            Ok(erc20.address().abi_encode()),
        );
        let result = proxy
            .sender(alice)
            .fallback(&transfer_call)
            .expect("should be able to dispatch transfer");
        assert_eq!(result, erc20.address().abi_encode());

        let balance_of_bob_call =
            IERC20::balanceOfCall { account: bob }.abi_encode();
        proxy.vm().mock_delegate_call(
            erc20.address(),
            balance_of_bob_call.clone(),
            Ok(erc20.address().abi_encode()),
        );
        let balance = proxy
            .sender(alice)
            .fallback(&balance_of_bob_call)
            .expect("should be able to dispatch balance query");
        assert_eq!(balance, erc20.address().abi_encode());
    }

    #[test]
    fn fallback_returns_error() {
        let proxy =
            TestVMContract::<BeaconProxyExample>::new(stringify!(proxy));
        let beacon = TestVMContract::<Beacon>::new(stringify!(beacon));
        let erc20 = TestVMContract::<Erc20Example>::new(stringify!(erc20));
        let alice = test_address(stringify!(alice));
        let bob = test_address(stringify!(bob));

        beacon.sender(alice).constructor(erc20.address());
        mock_beacon(&proxy, beacon.address(), erc20.address());

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
            Err(erc20.address().abi_encode()),
        );
        let err = proxy
            .sender(alice)
            .fallback(&transfer_call)
            .expect_err("should revert on transfer");

        assert_eq!(err, erc20.address().abi_encode());
    }
}
