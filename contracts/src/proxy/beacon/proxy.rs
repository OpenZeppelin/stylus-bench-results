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
    /// * [`Error::NonPayable`] - If the data is empty and
    ///   `self.vm().msg_value()` is not [`U256::ZERO`][U256].
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

// Not built with `export-abi`, see `proxy::test_helpers`.
#[cfg(all(test, not(feature = "export-abi")))]
mod tests {
    #![allow(clippy::needless_pass_by_value)]

    use alloy_sol_macro::sol;
    use alloy_sol_types::{SolCall, SolValue};
    use stylus_sdk::{
        alloy_primitives::{address, Address, U256},
        function_selector,
        prelude::*,
        testing::TestVM,
        ArbResult,
    };

    use super::*;
    use crate::{
        proxy::{
            beacon::IBeacon,
            test_helpers::{assert_emitted, deploy, execute},
        },
        test_utils::{ALICE, BOB},
        token::erc20::{self, Erc20, IErc20},
    };

    /// The beacon the proxy is created with.
    const BEACON: Address =
        address!("0x000000000000000000000000000000000000be4c");
    /// The implementation the beacon points to.
    const ERC20: Address =
        address!("0x000000000000000000000000000000000000e200");

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
        ) -> Result<bool, erc20::Error> {
            self.erc20.transfer(to, value)
        }

        fn transfer_from(
            &mut self,
            from: Address,
            to: Address,
            value: U256,
        ) -> Result<bool, erc20::Error> {
            self.erc20.transfer_from(from, to, value)
        }

        fn allowance(&self, owner: Address, spender: Address) -> U256 {
            self.erc20.allowance(owner, spender)
        }

        fn approve(
            &mut self,
            spender: Address,
            value: U256,
        ) -> Result<bool, erc20::Error> {
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

    // `TestVM` answers every call with the return data of the last mock
    // registered, whichever call the mock is for. The proxy makes two calls to
    // serve a call to its fallback: it asks the beacon for the implementation
    // (`mock_beacon`), then delegates the call to it (`mock_implementation`).
    // So the tests register last the mock whose data they need: the one of the
    // implementation when they read the result of the call, and the one of the
    // beacon (that has to be decoded as an address) otherwise.

    /// Makes `vm` see a contract at [`BEACON`], that answers `implementation()`
    /// the way [`Beacon`] does when running over the state of `beacon_vm`.
    fn mock_beacon(vm: &TestVM, beacon_vm: &TestVM) {
        deploy(vm, BEACON);

        let calldata = function_selector!("implementation").to_vec();
        vm.mock_static_call(
            BEACON,
            calldata.clone(),
            execute::<Beacon>(beacon_vm, &calldata),
        );
    }

    /// Mocks the delegate call of the proxy with `calldata` to [`ERC20`], with
    /// the outcome of running [`Erc20Example`] over the storage of the proxy.
    fn mock_implementation(vm: &TestVM, calldata: &[u8]) {
        vm.mock_delegate_call(
            ERC20,
            calldata.to_vec(),
            execute::<Erc20Example>(vm, calldata),
        );
    }

    #[test]
    fn constructs() {
        let vm = TestVM::default();
        let mut proxy = BeaconProxyExample::from(&vm);
        vm.set_sender(ALICE);
        let beacon_vm = TestVM::default();
        let mut beacon = Beacon::from(&beacon_vm);
        deploy(&vm, ERC20);

        beacon.constructor(ERC20);
        mock_beacon(&vm, &beacon_vm);

        proxy.constructor(BEACON, vec![].into()).unwrap();

        let implementation = proxy
            .implementation()
            .expect("should be able to get implementation");
        assert_eq!(implementation, ERC20);

        let beacon_address = proxy.get_beacon();
        assert_eq!(beacon_address, BEACON);
    }

    #[test]
    fn constructs_with_data() {
        let vm = TestVM::default();
        let mut proxy = BeaconProxyExample::from(&vm);
        vm.set_sender(ALICE);
        let beacon_vm = TestVM::default();
        let mut beacon = Beacon::from(&beacon_vm);
        deploy(&vm, ERC20);

        beacon.constructor(ERC20);

        let amount = U256::from(1000);

        let data = IERC20::mintCall { to: ALICE, value: amount }.abi_encode();

        // the constructor delegates the data to the implementation.
        mock_implementation(&vm, &data);
        mock_beacon(&vm, &beacon_vm);
        proxy
            .constructor(BEACON, data.into())
            .expect("should be able to construct");

        let implementation = proxy
            .implementation()
            .expect("should be able to get implementation");
        assert_eq!(implementation, ERC20);

        let beacon_address = proxy.get_beacon();
        assert_eq!(beacon_address, BEACON);

        let balance_of_alice_call =
            IERC20::balanceOfCall { account: ALICE }.abi_encode();
        mock_implementation(&vm, &balance_of_alice_call);
        let balance = proxy
            .fallback(&balance_of_alice_call)
            .expect("should be able to get balance");
        assert_eq!(balance, amount.abi_encode());

        let total_supply_call = IERC20::totalSupplyCall {}.abi_encode();
        mock_implementation(&vm, &total_supply_call);
        let total_supply = proxy
            .fallback(&total_supply_call)
            .expect("should be able to get total supply");
        assert_eq!(total_supply, amount.abi_encode());
    }

    #[test]
    fn fallback() {
        let vm = TestVM::default();
        let mut proxy = BeaconProxyExample::from(&vm);
        vm.set_sender(ALICE);
        let beacon_vm = TestVM::default();
        let mut beacon = Beacon::from(&beacon_vm);
        deploy(&vm, ERC20);

        beacon.constructor(ERC20);
        mock_beacon(&vm, &beacon_vm);

        proxy
            .constructor(BEACON, vec![].into())
            .expect("should be able to construct");

        // verify initial balance is [`U256::ZERO`].
        let balance_of_alice_call =
            IERC20::balanceOfCall { account: ALICE }.abi_encode();
        mock_implementation(&vm, &balance_of_alice_call);
        let balance = proxy
            .fallback(&balance_of_alice_call)
            .expect("should be able to get balance");
        assert_eq!(balance, U256::ZERO.abi_encode());

        let total_supply_call = IERC20::totalSupplyCall {}.abi_encode();
        mock_implementation(&vm, &total_supply_call);
        let total_supply = proxy
            .fallback(&total_supply_call)
            .expect("should be able to get total supply");
        assert_eq!(total_supply, U256::ZERO.abi_encode());

        // mint 1000 tokens.
        let amount = U256::from(1000);

        let mint_call =
            IERC20::mintCall { to: ALICE, value: amount }.abi_encode();
        mock_implementation(&vm, &mint_call);
        mock_beacon(&vm, &beacon_vm);
        proxy.fallback(&mint_call).expect("should be able to mint");
        // the implementation runs in the context of the proxy, so the proxy is
        // the one that emits the event.
        assert_emitted(
            &vm,
            &erc20::Transfer { from: Address::ZERO, to: ALICE, value: amount },
        );

        // check that the balance can be accurately fetched through the proxy.
        mock_implementation(&vm, &balance_of_alice_call);
        let balance = proxy
            .fallback(&balance_of_alice_call)
            .expect("should be able to get balance");
        assert_eq!(balance, amount.abi_encode());

        mock_implementation(&vm, &total_supply_call);
        let total_supply = proxy
            .fallback(&total_supply_call)
            .expect("should be able to get total supply");
        assert_eq!(total_supply, amount.abi_encode());

        // check that the balance can be transferred through the proxy.
        let transfer_call =
            IERC20::transferCall { to: BOB, value: amount }.abi_encode();
        mock_implementation(&vm, &transfer_call);
        mock_beacon(&vm, &beacon_vm);
        proxy.fallback(&transfer_call).expect("should be able to transfer");

        assert_emitted(
            &vm,
            &erc20::Transfer { from: ALICE, to: BOB, value: amount },
        );

        mock_implementation(&vm, &balance_of_alice_call);
        let balance = proxy
            .fallback(&balance_of_alice_call)
            .expect("should be able to get balance");
        assert_eq!(balance, U256::ZERO.abi_encode());

        let balance_of_bob_call =
            IERC20::balanceOfCall { account: BOB }.abi_encode();
        mock_implementation(&vm, &balance_of_bob_call);
        let balance = proxy
            .fallback(&balance_of_bob_call)
            .expect("should be able to get balance");
        assert_eq!(balance, amount.abi_encode());

        mock_implementation(&vm, &total_supply_call);
        let total_supply = proxy
            .fallback(&total_supply_call)
            .expect("should be able to get total supply");
        assert_eq!(total_supply, amount.abi_encode());
    }

    #[test]
    fn fallback_returns_error() {
        let vm = TestVM::default();
        let mut proxy = BeaconProxyExample::from(&vm);
        vm.set_sender(ALICE);
        let beacon_vm = TestVM::default();
        let mut beacon = Beacon::from(&beacon_vm);
        deploy(&vm, ERC20);

        beacon.constructor(ERC20);
        mock_beacon(&vm, &beacon_vm);

        proxy
            .constructor(BEACON, vec![].into())
            .expect("should be able to construct");

        let amount = U256::from(1000);
        let transfer_call =
            IERC20::transferCall { to: BOB, value: amount }.abi_encode();
        // The data the proxy gets back from the implementation is the data of
        // the last mock, which is also what the beacon answers with. So the
        // implementation reverts with the encoded address of the
        // implementation, the only data that serves for both calls.
        let reason = ERC20.abi_encode();
        vm.mock_delegate_call(
            ERC20,
            transfer_call.clone(),
            Err(reason.clone()),
        );
        mock_beacon(&vm, &beacon_vm);
        let err = proxy
            .fallback(&transfer_call)
            .expect_err("should revert on transfer");

        assert_eq!(err, reason);
    }
}
