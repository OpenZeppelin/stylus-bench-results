//! This library provides getters and event emitting update functions for
//! [ERC-1967] slots.
//!
//! [ERC-1967]: https://eips.ethereum.org/EIPS/eip-1967

use alloy_primitives::{aliases::B256, uint, Address, U256};
pub use sol::*;
use stylus_sdk::{
    abi::Bytes, host::VM, prelude::*, storage::StorageAddress,
    stylus_core::calls::errors::MethodError,
};

use crate::{
    proxy::{beacon::IBeaconInterface, erc1967},
    utils::{
        address::{self, AddressUtils},
        storage_slot::StorageSlot,
    },
};

#[cfg_attr(coverage_nightly, coverage(off))]
mod sol {
    use alloy_sol_macro::sol;

    sol! {
        /// Indicates an error related to the fact that the `implementation`
        /// of the proxy is invalid.
        ///
        /// * `implementation` - Address of the invalid implementation.
        #[derive(Debug)]
        #[allow(missing_docs)]
        error ERC1967InvalidImplementation(address implementation);

        /// Indicates an error related to the fact that the `admin` of the
        /// proxy is invalid.
        ///
        /// * `admin` - Address of the invalid admin.
        #[derive(Debug)]
        #[allow(missing_docs)]
        error ERC1967InvalidAdmin(address admin);

        /// Indicates an error related to the fact that the `beacon`
        /// of the proxy is invalid.
        ///
        /// * `beacon` - Address of the invalid `beacon` of the proxy.
        #[derive(Debug)]
        #[allow(missing_docs)]
        error ERC1967InvalidBeacon(address beacon);

        /// Indicates an error related to the fact that an upgrade function
        /// sees `self.vm().msg_value()` > [`alloy_primitives::U256::ZERO`]
        /// that may be lost.
        #[derive(Debug)]
        #[allow(missing_docs)]
        error ERC1967NonPayable();
    }
}

/// An [`Erc1967Utils`] error.
#[derive(SolidityError, Debug)]
pub enum Error {
    /// Indicates an error related to the fact that the `implementation`
    /// of the proxy is invalid.
    InvalidImplementation(ERC1967InvalidImplementation),
    /// Indicates an error related to the fact that the `admin` of the
    /// proxy is invalid.
    InvalidAdmin(ERC1967InvalidAdmin),
    /// Indicates an error related to the fact that the `beacon`
    /// of the proxy is invalid.
    InvalidBeacon(ERC1967InvalidBeacon),
    /// Indicates an error related to the fact that an upgrade function
    /// sees `self.vm().msg_value()` > [`alloy_primitives::U256::ZERO`]
    /// that may be lost.
    NonPayable(ERC1967NonPayable),
    /// There's no code at `target` (it is not a contract).
    EmptyCode(address::AddressEmptyCode),
    /// A call to an address target failed. The target may have reverted.
    FailedCall(address::FailedCall),
    /// Indicates an error related to the fact that the delegate call
    /// failed.
    FailedCallWithReason(address::FailedCallWithReason),
}

impl From<address::Error> for Error {
    fn from(e: address::Error) -> Self {
        match e {
            address::Error::EmptyCode(e) => Error::EmptyCode(e),
            address::Error::FailedCall(e) => Error::FailedCall(e),
            address::Error::FailedCallWithReason(e) => {
                Error::FailedCallWithReason(e)
            }
        }
    }
}

impl MethodError for Error {
    fn encode(self) -> alloc::vec::Vec<u8> {
        self.into()
    }
}

/// Storage slot with the address of the current implementation.
pub const IMPLEMENTATION_SLOT: B256 = {
    const HASH: [u8; 32] = keccak_const::Keccak256::new()
        .update(b"eip1967.proxy.implementation")
        .finalize();
    B256::new(
        U256::from_be_bytes(HASH).wrapping_sub(uint!(1_U256)).to_be_bytes(),
    )
};

/// Storage slot with the admin of the contract.
pub const ADMIN_SLOT: B256 = {
    const HASH: [u8; 32] = keccak_const::Keccak256::new()
        .update(b"eip1967.proxy.admin")
        .finalize();
    B256::new(
        U256::from_be_bytes(HASH).wrapping_sub(uint!(1_U256)).to_be_bytes(),
    )
};

/// The storage slot of the beacon contract which defines the implementation
/// for this proxy.
pub const BEACON_SLOT: B256 = {
    const HASH: [u8; 32] = keccak_const::Keccak256::new()
        .update(b"eip1967.proxy.beacon")
        .finalize();
    B256::new(
        U256::from_be_bytes(HASH).wrapping_sub(uint!(1_U256)).to_be_bytes(),
    )
};

/// This library provides getters and event emitting update functions for
/// [ERC-1967] slots.
///
/// [ERC-1967]: https://eips.ethereum.org/EIPS/eip-1967
pub struct Erc1967Utils;

/// Implementation of the [`Erc1967Utils`] library.
impl Erc1967Utils {
    /// Returns the current implementation address.
    ///
    /// # Arguments
    ///
    /// * `context` - Read access to the contract's state.
    #[must_use]
    pub fn get_implementation(context: &impl HostAccess<Host = VM>) -> Address {
        StorageSlot::get_slot::<StorageAddress>(context, IMPLEMENTATION_SLOT)
            .get()
    }

    /// Performs implementation upgrade with additional setup call if
    /// data is nonempty. This function is payable only if the setup call
    /// is performed, otherwise `context.vm().msg_value()` is rejected to avoid
    /// stuck value in the contract.
    ///
    /// # Arguments
    ///
    /// * `context` - Mutable access to the contract's state.
    /// * `new_implementation` - The new implementation address.
    /// * `data` - The data to pass to the setup call.
    ///
    /// # Errors
    ///
    /// * [`Error::InvalidImplementation`] - If the `new_implementation` address
    ///   is not a valid implementation.
    /// * [`Error::NonPayable`] - If `data` is empty and
    ///   `context.vm().msg_value()` is not [`U256::ZERO`].
    /// * [`Error::FailedCall`] - If the call to the implementation contract
    ///   fails.
    /// * [`Error::FailedCallWithReason`] - If the call to the implementation
    ///   contract fails with a revert reason.
    pub fn upgrade_to_and_call<T: TopLevelStorage + HostAccess<Host = VM>>(
        context: &mut T,
        new_implementation: Address,
        data: &Bytes,
    ) -> Result<(), Error> {
        Erc1967Utils::set_implementation(context, new_implementation)?;

        context
            .vm()
            .log(erc1967::Upgraded { implementation: new_implementation });

        if data.is_empty() {
            Erc1967Utils::check_non_payable(context)?;
        } else {
            AddressUtils::function_delegate_call(
                context,
                new_implementation,
                data.as_ref(),
            )?;
        }

        Ok(())
    }

    /// Returns the current admin.
    ///
    /// # Arguments
    ///
    /// * `context` - Read access to the contract's state.
    #[must_use]
    pub fn get_admin(context: &impl HostAccess<Host = VM>) -> Address {
        StorageSlot::get_slot::<StorageAddress>(context, ADMIN_SLOT).get()
    }

    /// Changes the admin of the proxy.
    ///
    /// # Arguments
    ///
    /// * `context` - Write access to the contract's state.
    /// * `new_admin` - The new admin address.
    ///
    /// # Errors
    ///
    /// * [`Error::InvalidAdmin`] - If the `new_admin` address is not a valid
    ///   admin.
    pub fn change_admin(
        context: &impl HostAccess<Host = VM>,
        new_admin: Address,
    ) -> Result<(), Error> {
        context.vm().log(erc1967::AdminChanged {
            previous_admin: Erc1967Utils::get_admin(context),
            new_admin,
        });

        Erc1967Utils::set_admin(context, new_admin)
    }

    /// Returns the current beacon.
    ///
    /// # Arguments
    ///
    /// * `context` - Read access to the contract's state.
    #[must_use]
    pub fn get_beacon(context: &impl HostAccess<Host = VM>) -> Address {
        StorageSlot::get_slot::<StorageAddress>(context, BEACON_SLOT).get()
    }

    /// Change the beacon and trigger a setup call if data is nonempty.
    /// This function is payable only if the setup call is performed,
    /// otherwise `context.vm().msg_value()` is rejected to avoid stuck value
    /// in the contract.
    ///
    /// # Arguments
    ///
    /// * `context` - Mutable access to the contract's state.
    /// * `new_beacon` - The new beacon address.
    /// * `data` - The data to pass to the setup call.
    ///
    /// # Errors
    ///
    /// * [`Error::InvalidBeacon`] - If the `new_beacon` address is not a valid
    ///   beacon.
    /// * [`Error::InvalidImplementation`] - If the beacon implementation is not
    ///   a valid implementation.
    /// * [`Error::NonPayable`] - If `data` is empty and
    ///   `context.vm().msg_value()` is not [`U256::ZERO`].
    /// * [`Error::FailedCall`] - If the call to the beacon implementation
    ///   fails.
    /// * [`Error::FailedCallWithReason`] - If the call to the beacon
    ///   implementation fails with a revert reason.
    pub fn upgrade_beacon_to_and_call<
        T: TopLevelStorage + HostAccess<Host = VM>,
    >(
        context: &mut T,
        new_beacon: Address,
        data: &Bytes,
    ) -> Result<(), Error> {
        Erc1967Utils::set_beacon(context, new_beacon)?;
        context.vm().log(erc1967::BeaconUpgraded { beacon: new_beacon });

        if data.is_empty() {
            Erc1967Utils::check_non_payable(context)?;
        } else {
            let beacon_implementation =
                Erc1967Utils::get_beacon_implementation(context, new_beacon)?;

            AddressUtils::function_delegate_call(
                context,
                beacon_implementation,
                data.as_ref(),
            )?;
        }

        Ok(())
    }
}

impl Erc1967Utils {
    /// Reverts if `context.vm().msg_value()` is not
    /// [`alloy_primitives::U256::ZERO`]. It can be used to avoid
    /// `context.vm().msg_value()` stuck in the contract if an upgrade does not
    /// perform an initialization call.
    ///
    /// # Arguments
    ///
    /// * `context` - Read access to the contract's state.
    ///
    /// # Errors
    ///
    /// * [`Error::NonPayable`] - If `context.vm().msg_value()` is not
    ///   [`alloy_primitives::U256::ZERO`].
    fn check_non_payable(
        context: &impl HostAccess<Host = VM>,
    ) -> Result<(), Error> {
        if context.vm().msg_value().is_zero() {
            Ok(())
        } else {
            Err(ERC1967NonPayable {}.into())
        }
    }

    /// Stores a new address in the ERC-1967 implementation slot.
    ///
    /// # Arguments
    ///
    /// * `context` - Write access to the contract's state.
    /// * `new_implementation` - The new implementation address.
    ///
    /// # Errors
    ///
    /// * [`Error::InvalidImplementation`] - If the `new_implementation` address
    ///   is not a valid implementation.
    fn set_implementation(
        context: &impl HostAccess<Host = VM>,
        new_implementation: Address,
    ) -> Result<(), Error> {
        if !AddressUtils::has_code(context, new_implementation) {
            return Err(ERC1967InvalidImplementation {
                implementation: new_implementation,
            }
            .into());
        }

        StorageSlot::get_slot::<StorageAddress>(context, IMPLEMENTATION_SLOT)
            .set(new_implementation);

        Ok(())
    }

    /// Stores a new address in the ERC-1967 admin slot.
    ///
    /// # Arguments
    ///
    /// * `context` - Write access to the contract's state.
    /// * `new_admin` - The new admin address.
    ///
    /// # Errors
    ///
    /// * [`Error::InvalidAdmin`] - If the `new_admin` address is not a valid
    ///   admin.
    fn set_admin(
        context: &impl HostAccess<Host = VM>,
        new_admin: Address,
    ) -> Result<(), Error> {
        if new_admin.is_zero() {
            return Err(ERC1967InvalidAdmin { admin: new_admin }.into());
        }

        StorageSlot::get_slot::<StorageAddress>(context, ADMIN_SLOT)
            .set(new_admin);

        Ok(())
    }

    /// Stores a new beacon in the ERC-1967 beacon slot.
    ///
    /// # Arguments
    ///
    /// * `context` - Mutable access to the contract's state.
    /// * `new_beacon` - The new beacon address.
    ///
    /// # Errors
    ///
    /// * [`Error::InvalidBeacon`] - If the `new_beacon` address is not a valid
    ///   beacon.
    /// * [`Error::InvalidImplementation`] - If the beacon implementation is not
    ///   a valid implementation.
    /// * [`Error::FailedCall`] - If the call to the beacon implementation
    ///   fails.
    /// * [`Error::FailedCallWithReason`] - If the call to the beacon
    ///   implementation fails with a revert reason.
    fn set_beacon<T: TopLevelStorage + HostAccess<Host = VM>>(
        context: &mut T,
        new_beacon: Address,
    ) -> Result<(), Error> {
        if !AddressUtils::has_code(context, new_beacon) {
            return Err(ERC1967InvalidBeacon { beacon: new_beacon }.into());
        }

        StorageSlot::get_slot::<StorageAddress>(context, BEACON_SLOT)
            .set(new_beacon);

        let beacon_implementation =
            Erc1967Utils::get_beacon_implementation(context, new_beacon)?;

        if !AddressUtils::has_code(context, beacon_implementation) {
            return Err(ERC1967InvalidImplementation {
                implementation: beacon_implementation,
            }
            .into());
        }

        Ok(())
    }
}

impl Erc1967Utils {
    /// Returns the implementation address of the beacon.
    ///
    /// # Arguments
    ///
    /// * `context` - Readonly access to the contract's state.
    /// * `beacon` - The beacon address.
    ///
    /// # Errors
    ///
    /// * [`Error::FailedCall`] - If the call to the beacon implementation
    ///   fails.
    /// * [`Error::FailedCallWithReason`] - If the call to the beacon
    ///   implementation fails with a revert reason.
    /// * [`Error::EmptyCode`] - If the beacon implementation has no code.
    fn get_beacon_implementation(
        context: &impl HostAccess<Host = VM>,
        beacon: Address,
    ) -> Result<Address, Error> {
        Ok(AddressUtils::verify_call_result_from_target(
            context,
            beacon,
            IBeaconInterface::new(beacon)
                .implementation(context.vm(), Call::new()),
        )?)
    }
}

// Not built with `export-abi`, see `proxy::test_helpers`.
#[cfg(all(test, not(feature = "export-abi")))]
mod tests {
    #![allow(clippy::needless_pass_by_value, clippy::unused_self)]

    use alloy_sol_types::{SolCall, SolValue};
    use stylus_sdk::{
        alloy_primitives::{address, Address, U256},
        alloy_sol_types::sol,
        function_selector,
        prelude::*,
        storage::StorageAddress,
        testing::TestVM,
    };

    use super::*;
    use crate::{
        proxy::{
            beacon::IBeacon,
            test_helpers::{assert_emitted, deploy, execute},
        },
        test_utils::{ALICE, BOB, CHARLIE},
    };

    /// The implementation the contract is upgraded to.
    const IMPLEMENTATION: Address =
        address!("0x000000000000000000000000000000000000e200");
    /// The beacons the contract is upgraded to.
    const BEACON: Address =
        address!("0x000000000000000000000000000000000000be4c");
    const BEACON_2: Address =
        address!("0x000000000000000000000000000000000000be4d");

    #[entrypoint]
    #[storage]
    struct TestContract;

    #[public]
    impl TestContract {
        // test functions that wrap [`Erc1967Utils`] methods.
        fn test_get_implementation(&self) -> Address {
            Erc1967Utils::get_implementation(self)
        }

        fn test_upgrade_to_and_call(
            &mut self,
            new_implementation: Address,
            data: Bytes,
        ) -> Result<(), Vec<u8>> {
            Ok(Erc1967Utils::upgrade_to_and_call(
                self,
                new_implementation,
                &data,
            )?)
        }

        fn test_get_admin(&self) -> Address {
            Erc1967Utils::get_admin(self)
        }

        fn test_change_admin(
            &mut self,
            new_admin: Address,
        ) -> Result<(), Vec<u8>> {
            Ok(Erc1967Utils::change_admin(self, new_admin)?)
        }

        fn test_get_beacon(&self) -> Address {
            Erc1967Utils::get_beacon(self)
        }

        fn test_upgrade_beacon_to_and_call(
            &mut self,
            new_beacon: Address,
            data: Bytes,
        ) -> Result<(), Vec<u8>> {
            Ok(Erc1967Utils::upgrade_beacon_to_and_call(
                self, new_beacon, &data,
            )?)
        }
    }

    // mock beacon contract for testing.
    #[storage]
    struct MockBeacon {
        implementation: StorageAddress,
    }

    unsafe impl TopLevelStorage for MockBeacon {}

    #[public]
    #[implements(IBeacon)]
    impl MockBeacon {
        #[constructor]
        fn constructor(&mut self, implementation: Address) {
            self.implementation.set(implementation);
        }
    }

    #[public]
    impl IBeacon for MockBeacon {
        fn implementation(&self) -> Result<Address, Vec<u8>> {
            Ok(self.implementation.get())
        }
    }

    sol! {
        #[derive(Debug)]
        #[allow(missing_docs)]
        error ImplementationSolidityError();

        #[derive(Debug)]
        #[allow(missing_docs)]
        event ImplementationEvent();
    }

    #[derive(SolidityError, Debug)]
    enum ImplementationError {
        ImplementationError(ImplementationSolidityError),
    }

    impl MethodError for ImplementationError {
        fn encode(self) -> Vec<u8> {
            self.into()
        }
    }

    #[storage]
    struct Implementation;

    unsafe impl TopLevelStorage for Implementation {}

    #[public]
    impl Implementation {
        fn emit_or_error(
            &mut self,
            should_error: bool,
        ) -> Result<(), ImplementationError> {
            if should_error {
                return Err(ImplementationError::ImplementationError(
                    ImplementationSolidityError {},
                ));
            }
            self.vm().log(ImplementationEvent {});
            Ok(())
        }
    }

    sol! {
        interface IImplementation {
            function emitOrError(bool should_error) external;
        }
    }

    // `TestVM` answers every call with the return data of the last mock
    // registered, whichever call the mock is for. Upgrading to a beacon with
    // setup data makes calls that need different data: the ones asking the
    // beacon for its implementation, which need the encoded address, and the
    // delegate call of the setup data. The tests register the beacon last, so
    // what the delegate call returns, or reverts with, is the encoded address
    // of the implementation.

    /// Makes `vm` see a contract at `beacon` that answers `implementation()`
    /// with `implementation`, the way a [`MockBeacon`] constructed with it
    /// does.
    fn deploy_beacon(vm: &TestVM, beacon: Address, implementation: Address) {
        let beacon_vm = TestVM::default();
        MockBeacon::from(&beacon_vm).constructor(implementation);

        deploy(vm, beacon);
        let calldata = function_selector!("implementation").to_vec();
        vm.mock_static_call(
            beacon,
            calldata.clone(),
            execute::<MockBeacon>(&beacon_vm, &calldata),
        );
    }

    #[test]
    fn get_implementation_returns_zero_by_default() {
        let vm = TestVM::default();
        let contract = TestContract::from(&vm);
        vm.set_sender(ALICE);

        let implementation = contract.test_get_implementation();
        assert_eq!(implementation, Address::ZERO);
    }

    #[test]
    fn upgrade_to_and_call_with_valid_implementation_and_empty_data() {
        let vm = TestVM::default();
        let mut contract = TestContract::from(&vm);
        vm.set_sender(ALICE);
        deploy(&vm, IMPLEMENTATION);

        contract
            .test_upgrade_to_and_call(IMPLEMENTATION, vec![].into())
            .expect("should be able to upgrade to valid implementation");

        let implementation_addr = contract.test_get_implementation();
        assert_eq!(implementation_addr, IMPLEMENTATION);

        // verify event was emitted.
        assert_emitted(
            &vm,
            &erc1967::Upgraded { implementation: IMPLEMENTATION },
        );
    }

    #[test]
    fn upgrade_to_and_call_with_valid_implementation_and_data() {
        let vm = TestVM::default();
        let mut contract = TestContract::from(&vm);
        vm.set_sender(ALICE);
        deploy(&vm, IMPLEMENTATION);

        let data = IImplementation::emitOrErrorCall { should_error: false }
            .abi_encode();
        // the data is delegated to the implementation, which runs in the
        // context of the contract.
        vm.mock_delegate_call(
            IMPLEMENTATION,
            data.clone(),
            execute::<Implementation>(&vm, &data),
        );
        contract.test_upgrade_to_and_call(IMPLEMENTATION, data.into()).expect(
            "should be able to upgrade to valid implementation with data",
        );

        let implementation_addr = contract.test_get_implementation();
        assert_eq!(implementation_addr, IMPLEMENTATION);

        // verify event was emitted.
        assert_emitted(
            &vm,
            &erc1967::Upgraded { implementation: IMPLEMENTATION },
        );

        // the implementation runs in the context of the contract that delegate
        // called the upgrade, so the contract is the one that emits the event.
        assert_emitted(&vm, &ImplementationEvent {});
    }

    #[test]
    fn upgrade_to_and_call_with_invalid_implementation() {
        let vm = TestVM::default();
        let mut contract = TestContract::from(&vm);
        vm.set_sender(ALICE);

        let invalid_address = ALICE; // address with no code.
        let err = contract
            .test_upgrade_to_and_call(invalid_address, vec![].into())
            .expect_err("should fail when upgrading to invalid implementation");

        assert_eq!(
            err,
            Error::InvalidImplementation(ERC1967InvalidImplementation {
                implementation: invalid_address
            })
            .encode()
        );

        // implementation should remain unchanged.
        let implementation = contract.test_get_implementation();
        assert_eq!(implementation, Address::ZERO);
    }

    #[test]
    fn upgrade_to_and_call_with_zero_implementation() {
        let vm = TestVM::default();
        let mut contract = TestContract::from(&vm);
        vm.set_sender(ALICE);

        let err = contract
            .test_upgrade_to_and_call(Address::ZERO, vec![].into())
            .expect_err("should fail when upgrading to zero implementation");

        assert_eq!(
            err,
            Error::InvalidImplementation(ERC1967InvalidImplementation {
                implementation: Address::ZERO
            })
            .encode()
        );
    }

    #[test]
    fn upgrade_to_and_call_with_same_implementation() {
        let vm = TestVM::default();
        let mut contract = TestContract::from(&vm);
        vm.set_sender(ALICE);
        deploy(&vm, IMPLEMENTATION);

        // first upgrade.
        contract
            .test_upgrade_to_and_call(IMPLEMENTATION, vec![].into())
            .expect("should be able to upgrade first time");

        // upgrade to same implementation.
        contract
            .test_upgrade_to_and_call(IMPLEMENTATION, vec![].into())
            .expect("should be able to upgrade to same implementation");

        let implementation_addr = contract.test_get_implementation();
        assert_eq!(implementation_addr, IMPLEMENTATION);

        // event should still be emitted.
        assert_emitted(
            &vm,
            &erc1967::Upgraded { implementation: IMPLEMENTATION },
        );
    }

    #[test]
    fn upgrade_to_and_call_with_delegate_call_failure() {
        let vm = TestVM::default();
        let mut contract = TestContract::from(&vm);
        vm.set_sender(ALICE);
        deploy(&vm, IMPLEMENTATION);

        let data = function_selector!("nonExistentFunction").to_vec();

        let vec = format!(
            "function not found for selector '{0}' and no fallback defined",
            u32::from_be_bytes(TryInto::try_into(data.clone()).unwrap())
        )
        .as_bytes()
        .to_vec();
        // the delegate call reverts with a reason.
        vm.mock_delegate_call(IMPLEMENTATION, data.clone(), Err(vec.clone()));

        let err = contract
            .test_upgrade_to_and_call(IMPLEMENTATION, data.into())
            .expect_err("should fail when delegate call fails");

        assert_eq!(
            err,
            Error::FailedCallWithReason(address::FailedCallWithReason {
                reason: stylus_sdk::stylus_core::calls::errors::Error::Revert(
                    vec
                )
                .encode()
                .into()
            })
            .encode(),
        );
    }

    #[test]
    fn upgrade_to_and_call_with_implementation_reverts() {
        let vm = TestVM::default();
        let mut contract = TestContract::from(&vm);
        vm.set_sender(ALICE);
        deploy(&vm, IMPLEMENTATION);

        let data = IImplementation::emitOrErrorCall { should_error: true }
            .abi_encode();
        vm.mock_delegate_call(
            IMPLEMENTATION,
            data.clone(),
            execute::<Implementation>(&vm, &data),
        );

        let err = contract
            .test_upgrade_to_and_call(IMPLEMENTATION, data.into())
            .expect_err("should fail when implementation reverts");

        assert_eq!(
            err,
            Error::FailedCallWithReason(address::FailedCallWithReason {
                reason: ImplementationError::ImplementationError(
                    ImplementationSolidityError {}
                )
                .encode()
                .into()
            })
            .encode()
        );
    }

    #[test]
    fn upgrade_to_and_call_reverts_with_empty_data_with_value() {
        let vm = TestVM::default();
        let mut contract = TestContract::from(&vm);
        vm.set_sender(ALICE);
        deploy(&vm, IMPLEMENTATION);

        vm.set_balance(ALICE, U256::from(1000));
        vm.set_value(U256::from(1000));

        let err = contract
            .test_upgrade_to_and_call(IMPLEMENTATION, vec![].into())
            .expect_err("should fail with ERC1967NonPayable");

        assert_eq!(err, Error::NonPayable(ERC1967NonPayable {}).encode());
    }

    #[test]
    fn get_admin_returns_zero_by_default() {
        let vm = TestVM::default();
        let contract = TestContract::from(&vm);
        vm.set_sender(ALICE);

        let admin = contract.test_get_admin();
        assert_eq!(admin, Address::ZERO);
    }

    #[test]
    fn change_admin_with_valid_address() {
        let vm = TestVM::default();
        let mut contract = TestContract::from(&vm);
        vm.set_sender(ALICE);

        contract
            .test_change_admin(BOB)
            .expect("should be able to change admin to valid address");

        let admin = contract.test_get_admin();
        assert_eq!(admin, BOB);

        // Verify event was emitted
        assert_emitted(
            &vm,
            &erc1967::AdminChanged {
                previous_admin: Address::ZERO,
                new_admin: BOB,
            },
        );

        // verify that changing admin to [`Address::ZERO`] fails.
        let err = contract
            .test_change_admin(Address::ZERO)
            .expect_err("should fail when changing admin to zero address");

        assert_eq!(
            err,
            Error::InvalidAdmin(ERC1967InvalidAdmin { admin: Address::ZERO })
                .encode()
        );

        // admin should remain unchanged.
        let admin = contract.test_get_admin();
        assert_eq!(admin, BOB);
    }

    #[test]
    fn change_admin_multiple_times() {
        let vm = TestVM::default();
        let mut contract = TestContract::from(&vm);
        vm.set_sender(ALICE);

        // first change.
        contract
            .test_change_admin(BOB)
            .expect("should be able to change admin first time");

        let admin = contract.test_get_admin();
        assert_eq!(admin, BOB);

        // second change.
        contract
            .test_change_admin(CHARLIE)
            .expect("should be able to change admin second time");

        let admin = contract.test_get_admin();
        assert_eq!(admin, CHARLIE);

        // verify events were emitted.
        assert_emitted(
            &vm,
            &erc1967::AdminChanged {
                previous_admin: Address::ZERO,
                new_admin: BOB,
            },
        );
        assert_emitted(
            &vm,
            &erc1967::AdminChanged { previous_admin: BOB, new_admin: CHARLIE },
        );
    }

    #[test]
    fn change_admin_with_same_address() {
        let vm = TestVM::default();
        let mut contract = TestContract::from(&vm);
        vm.set_sender(ALICE);

        // first change.
        contract
            .test_change_admin(BOB)
            .expect("should be able to change admin first time");

        // change to same address.
        contract
            .test_change_admin(BOB)
            .expect("should be able to change admin to same address");

        let admin = contract.test_get_admin();
        assert_eq!(admin, BOB);

        // event should still be emitted.
        assert_emitted(
            &vm,
            &erc1967::AdminChanged {
                previous_admin: Address::ZERO,
                new_admin: BOB,
            },
        );
        assert_emitted(
            &vm,
            &erc1967::AdminChanged { previous_admin: BOB, new_admin: BOB },
        );
    }

    #[test]
    fn get_beacon_returns_zero_by_default() {
        let vm = TestVM::default();
        let contract = TestContract::from(&vm);
        vm.set_sender(ALICE);

        let beacon = contract.test_get_beacon();
        assert_eq!(beacon, Address::ZERO);
    }

    #[test]
    fn upgrade_beacon_to_and_call_with_valid_beacon_and_empty_data() {
        let vm = TestVM::default();
        let mut contract = TestContract::from(&vm);
        vm.set_sender(ALICE);
        deploy(&vm, IMPLEMENTATION);
        deploy_beacon(&vm, BEACON, IMPLEMENTATION);

        contract
            .test_upgrade_beacon_to_and_call(BEACON, vec![].into())
            .expect("should be able to upgrade to valid beacon");

        let beacon_address = contract.test_get_beacon();
        assert_eq!(beacon_address, BEACON);

        // verify event was emitted.
        assert_emitted(&vm, &erc1967::BeaconUpgraded { beacon: BEACON });
    }

    #[test]
    fn upgrade_beacon_to_and_call_with_valid_beacon_and_data() {
        let vm = TestVM::default();
        let mut contract = TestContract::from(&vm);
        vm.set_sender(ALICE);
        deploy(&vm, IMPLEMENTATION);

        let data = IImplementation::emitOrErrorCall { should_error: false }
            .abi_encode();
        // the data is delegated to the implementation of the beacon, which
        // runs in the context of the contract.
        vm.mock_delegate_call(
            IMPLEMENTATION,
            data.clone(),
            execute::<Implementation>(&vm, &data),
        );
        deploy_beacon(&vm, BEACON, IMPLEMENTATION);
        contract
            .test_upgrade_beacon_to_and_call(BEACON, data.into())
            .expect("should be able to upgrade to valid beacon with data");

        let beacon_address = contract.test_get_beacon();
        assert_eq!(beacon_address, BEACON);

        // verify event was emitted.
        assert_emitted(&vm, &erc1967::BeaconUpgraded { beacon: BEACON });

        // the implementation runs in the context of the contract that delegate
        // called the upgrade, so the contract is the one that emits the event.
        assert_emitted(&vm, &ImplementationEvent {});
    }

    #[test]
    fn upgrade_beacon_to_and_call_with_invalid_beacon() {
        let vm = TestVM::default();
        let mut contract = TestContract::from(&vm);
        vm.set_sender(ALICE);

        let invalid_address = ALICE; // address with no code.
        let err = contract
            .test_upgrade_beacon_to_and_call(invalid_address, vec![].into())
            .expect_err("should fail when upgrading to invalid beacon");

        assert_eq!(
            err,
            Error::InvalidBeacon(ERC1967InvalidBeacon {
                beacon: invalid_address
            })
            .encode()
        );

        // beacon should remain unchanged.
        let beacon = contract.test_get_beacon();
        assert_eq!(beacon, Address::ZERO);
    }

    #[test]
    fn upgrade_beacon_to_and_call_with_zero_beacon() {
        let vm = TestVM::default();
        let mut contract = TestContract::from(&vm);
        vm.set_sender(ALICE);
        deploy(&vm, IMPLEMENTATION);
        deploy_beacon(&vm, BEACON, IMPLEMENTATION);

        contract
            .test_upgrade_beacon_to_and_call(BEACON, vec![].into())
            .expect("should be able to upgrade to valid beacon");

        let err = contract
            .test_upgrade_beacon_to_and_call(Address::ZERO, vec![].into())
            .expect_err("should fail when upgrading to zero beacon");

        assert_eq!(
            err,
            Error::InvalidBeacon(ERC1967InvalidBeacon {
                beacon: Address::ZERO
            })
            .encode()
        );
    }

    #[test]
    #[ignore = "TestVM doesn't roll back the storage writes of a call that fails: the beacon slot is written before the implementation of the beacon is checked, and stays set"]
    fn upgrade_beacon_to_and_call_with_beacon_returning_invalid_implementation()
    {
        let vm = TestVM::default();
        let mut contract = TestContract::from(&vm);
        vm.set_sender(ALICE);

        // beacon returns an address with no code.
        let invalid_address = ALICE; // address with no code.
        deploy_beacon(&vm, BEACON, invalid_address);

        let err = contract
            .test_upgrade_beacon_to_and_call(BEACON, vec![].into())
            .expect_err(
                "should fail when beacon returns invalid implementation",
            );

        assert_eq!(
            err,
            Error::InvalidImplementation(ERC1967InvalidImplementation {
                implementation: invalid_address
            })
            .encode()
        );

        // beacon should remain unchanged.
        let beacon_address = contract.test_get_beacon();
        assert_eq!(beacon_address, Address::ZERO);
    }

    #[test]
    fn upgrade_beacon_to_and_call_with_beacon_returning_zero_implementation() {
        let vm = TestVM::default();
        let mut contract = TestContract::from(&vm);
        vm.set_sender(ALICE);

        // beacon returns [`Address::ZERO`].
        deploy_beacon(&vm, BEACON, Address::ZERO);

        let err = contract
            .test_upgrade_beacon_to_and_call(BEACON, vec![].into())
            .expect_err("should fail when beacon returns zero implementation");

        assert_eq!(
            err,
            Error::InvalidImplementation(ERC1967InvalidImplementation {
                implementation: Address::ZERO
            })
            .encode()
        );
    }

    #[test]
    fn upgrade_beacon_to_and_call_multiple_times() {
        let vm = TestVM::default();
        let mut contract = TestContract::from(&vm);
        vm.set_sender(ALICE);
        deploy(&vm, IMPLEMENTATION);
        deploy_beacon(&vm, BEACON, IMPLEMENTATION);
        deploy_beacon(&vm, BEACON_2, IMPLEMENTATION);

        // first upgrade.
        contract
            .test_upgrade_beacon_to_and_call(BEACON, vec![].into())
            .expect("should be able to upgrade beacon first time");

        let beacon = contract.test_get_beacon();
        assert_eq!(beacon, BEACON);

        // second upgrade.
        contract
            .test_upgrade_beacon_to_and_call(BEACON_2, vec![].into())
            .expect("should be able to upgrade beacon second time");

        let beacon = contract.test_get_beacon();
        assert_eq!(beacon, BEACON_2);

        // verify events were emitted.
        assert_emitted(&vm, &erc1967::BeaconUpgraded { beacon: BEACON });
        assert_emitted(&vm, &erc1967::BeaconUpgraded { beacon: BEACON_2 });
    }

    #[test]
    fn upgrade_beacon_to_and_call_with_same_beacon() {
        let vm = TestVM::default();
        let mut contract = TestContract::from(&vm);
        vm.set_sender(ALICE);
        deploy(&vm, IMPLEMENTATION);
        deploy_beacon(&vm, BEACON, IMPLEMENTATION);

        // first upgrade.
        contract
            .test_upgrade_beacon_to_and_call(BEACON, vec![].into())
            .expect("should be able to upgrade beacon first time");

        // upgrade to same beacon.
        contract
            .test_upgrade_beacon_to_and_call(BEACON, vec![].into())
            .expect("should be able to upgrade to same beacon");

        let beacon_address = contract.test_get_beacon();
        assert_eq!(beacon_address, BEACON);

        // event should still be emitted.
        assert_emitted(&vm, &erc1967::BeaconUpgraded { beacon: BEACON });
    }

    #[test]
    fn upgrade_beacon_to_and_call_with_delegate_call_failure() {
        let vm = TestVM::default();
        let mut contract = TestContract::from(&vm);
        vm.set_sender(ALICE);
        deploy(&vm, IMPLEMENTATION);

        let data = function_selector!("nonExistentFunction").to_vec();

        // the delegate call reverts with a reason, which has to be the encoded
        // address the beacon answers with (see `deploy_beacon`).
        let vec = IMPLEMENTATION.abi_encode();
        vm.mock_delegate_call(IMPLEMENTATION, data.clone(), Err(vec.clone()));
        deploy_beacon(&vm, BEACON, IMPLEMENTATION);

        let err = contract
            .test_upgrade_beacon_to_and_call(BEACON, data.into())
            .expect_err("should fail when delegate call fails");

        assert_eq!(
            err,
            Error::FailedCallWithReason(address::FailedCallWithReason {
                reason: stylus_sdk::stylus_core::calls::errors::Error::Revert(
                    vec
                )
                .encode()
                .into()
            })
            .encode(),
        );
    }

    #[test]
    fn upgrade_beacon_to_and_call_with_beacon_implementation_reverts() {
        let vm = TestVM::default();
        let mut contract = TestContract::from(&vm);
        vm.set_sender(ALICE);
        deploy(&vm, IMPLEMENTATION);

        let data = IImplementation::emitOrErrorCall { should_error: true }
            .abi_encode();

        // the implementation reverts with a reason, which has to be the
        // encoded address the beacon answers with (see `deploy_beacon`).
        let reason = IMPLEMENTATION.abi_encode();
        vm.mock_delegate_call(
            IMPLEMENTATION,
            data.clone(),
            Err(reason.clone()),
        );
        deploy_beacon(&vm, BEACON, IMPLEMENTATION);

        let err = contract
            .test_upgrade_beacon_to_and_call(BEACON, data.into())
            .expect_err("should fail when beacon implementation reverts");

        assert_eq!(
            err,
            Error::FailedCallWithReason(address::FailedCallWithReason {
                reason: reason.into()
            })
            .encode()
        );
    }

    #[test]
    fn upgrade_beacon_to_and_call_reverts_with_empty_data_with_value() {
        let vm = TestVM::default();
        let mut contract = TestContract::from(&vm);
        vm.set_sender(ALICE);
        deploy(&vm, IMPLEMENTATION);
        deploy_beacon(&vm, BEACON, IMPLEMENTATION);

        vm.set_balance(ALICE, U256::from(1000));
        vm.set_value(U256::from(1000));

        let err = contract
            .test_upgrade_beacon_to_and_call(BEACON, vec![].into())
            .expect_err("should fail with ERC1967NonPayable");

        assert_eq!(err, Error::NonPayable(ERC1967NonPayable {}).encode());
    }

    // test storage slot isolation.
    #[test]
    fn storage_slots_are_independent() {
        let vm = TestVM::default();
        let mut contract = TestContract::from(&vm);
        vm.set_sender(ALICE);
        deploy(&vm, IMPLEMENTATION);
        deploy_beacon(&vm, BEACON, IMPLEMENTATION);

        // set implementation.
        contract
            .test_upgrade_to_and_call(IMPLEMENTATION, vec![].into())
            .expect("should be able to set implementation");

        // set admin.
        contract.test_change_admin(BOB).expect("should be able to set admin");

        // set beacon.
        contract
            .test_upgrade_beacon_to_and_call(BEACON, vec![].into())
            .expect("should be able to set beacon");

        // verify all are set correctly.
        let implementation_addr = contract.test_get_implementation();
        let admin = contract.test_get_admin();
        let beacon_address = contract.test_get_beacon();

        assert_eq!(implementation_addr, IMPLEMENTATION);
        assert_eq!(admin, BOB);
        assert_eq!(beacon_address, BEACON);
    }
}
