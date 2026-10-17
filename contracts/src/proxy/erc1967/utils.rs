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

        /// Indicates an error relatoed to the fact that an upgrade function
        /// sees [`self.vm().msg_value()`] > [`U256::ZERO`] that may be lost.
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
    /// Indicates an error relatoed to the fact that an upgrade function
    /// sees [`self.vm().msg_value()`] > [`alloy_primitives::U256::ZERO`]
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
    #[must_use]
    pub fn get_implementation(context: &impl HostAccess<Host = VM>) -> Address {
        StorageSlot::get_slot::<StorageAddress>(context, IMPLEMENTATION_SLOT)
            .get()
    }

    /// Performs implementation upgrade with additional setup call if
    /// data is nonempty. This function is payable only if the setup call
    /// is performed, otherwise [`self.vm().msg_value()`] is rejected to avoid stuck
    /// value in the contract.
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
    /// * [`Error::NonPayable`] - If `data` is empty and the VM call value is not
    ///   [`U256::ZERO`].
    /// * [`Error::FailedCall`] - If the call to the implementation contract
    ///   fails.
    /// * [`Error::FailedCallWithReason`] - If the call to the implementation
    ///   contract fails with a revert reason.
    pub fn upgrade_to_and_call<T: TopLevelStorage + HostAccess<Host = VM>>(
        context: &mut T,
        new_implementation: Address,
        data: &Bytes,
    ) -> Result<(), Error> {
        let previous_implementation =
            Erc1967Utils::get_implementation(context);
        Erc1967Utils::set_implementation(context, new_implementation)?;

        if data.is_empty() {
            if let Err(error) = Erc1967Utils::check_non_payable(context) {
                StorageSlot::get_slot::<StorageAddress>(
                    context,
                    IMPLEMENTATION_SLOT,
                )
                .set(previous_implementation);
                return Err(error);
            }
        } else {
            if let Err(error) = AddressUtils::function_delegate_call(
                context,
                new_implementation,
                data.as_ref(),
            ) {
                StorageSlot::get_slot::<StorageAddress>(
                    context,
                    IMPLEMENTATION_SLOT,
                )
                .set(previous_implementation);
                return Err(error.into());
            }
        }

        context
            .vm()
            .log(erc1967::Upgraded { implementation: new_implementation });

        Ok(())
    }

    /// Returns the current admin.
    #[must_use]
    pub fn get_admin(context: &impl HostAccess<Host = VM>) -> Address {
        StorageSlot::get_slot::<StorageAddress>(context, ADMIN_SLOT).get()
    }

    /// Changes the admin of the proxy.
    ///
    /// # Arguments
    ///
    /// * `new_admin` - The new admin address.
    ///
    /// # Errors
    ///
    /// * [`Error::InvalidAdmin`] - If the `new_admin` address is not a valid
    ///   admin.
    pub fn change_admin<T: HostAccess<Host = VM>>(
        context: &T,
        new_admin: Address,
    ) -> Result<(), Error> {
        let previous_admin = Erc1967Utils::get_admin(context);
        Erc1967Utils::set_admin(context, new_admin)?;

        context.vm().log(erc1967::AdminChanged {
            previous_admin,
            new_admin,
        });

        Ok(())
    }

    /// Returns the current beacon.
    #[must_use]
    pub fn get_beacon(context: &impl HostAccess<Host = VM>) -> Address {
        StorageSlot::get_slot::<StorageAddress>(context, BEACON_SLOT).get()
    }

    /// Change the beacon and trigger a setup call if data is nonempty.
    /// This function is payable only if the setup call is performed,
    /// otherwise [`self.vm().msg_value()`] is rejected to avoid stuck value in the
    /// contract.
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
    /// * [`Error::NonPayable`] - If `data` is empty and the VM call value is not
    ///   [`U256::ZERO`].
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
        let previous_beacon = Erc1967Utils::get_beacon(context);
        let beacon_implementation =
            Erc1967Utils::set_beacon(context, new_beacon)?;

        if data.is_empty() {
            if let Err(error) = Erc1967Utils::check_non_payable(context) {
                StorageSlot::get_slot::<StorageAddress>(context, BEACON_SLOT)
                    .set(previous_beacon);
                return Err(error);
            }
        } else {
            if let Err(error) = AddressUtils::function_delegate_call(
                context,
                beacon_implementation,
                data.as_ref(),
            ) {
                StorageSlot::get_slot::<StorageAddress>(context, BEACON_SLOT)
                    .set(previous_beacon);
                return Err(error.into());
            }
        }

        context.vm().log(erc1967::BeaconUpgraded { beacon: new_beacon });

        Ok(())
    }
}

impl Erc1967Utils {
    /// Reverts if [`self.vm().msg_value()`] is not [`alloy_primitives::U256::ZERO`]. It
    /// can be used to avoid [`self.vm().msg_value()`] stuck in the contract if an
    /// upgrade does not perform an initialization call.
    ///
    /// # Errors
    ///
    /// * [`Error::NonPayable`] - If [`self.vm().msg_value()`] is not
    ///   [`alloy_primitives::U256::ZERO`].
    fn check_non_payable<T: HostAccess<Host = VM>>(
        context: &T,
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
    /// * `new_implementation` - The new implementation address.
    ///
    /// # Errors
    ///
    /// * [`Error::InvalidImplementation`] - If the `new_implementation` address
    ///   is not a valid implementation.
    fn set_implementation<T: HostAccess<Host = VM>>(
        context: &T,
        new_implementation: Address,
    ) -> Result<(), Error> {
        if context.vm().code_size(new_implementation) == 0 {
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
    /// * `new_admin` - The new admin address.
    ///
    /// # Errors
    ///
    /// * [`Error::InvalidAdmin`] - If the `new_admin` address is not a valid
    ///   admin.
    fn set_admin<T: HostAccess<Host = VM>>(
        context: &T,
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
    ) -> Result<Address, Error> {
        if context.vm().code_size(new_beacon) == 0 {
            return Err(ERC1967InvalidBeacon { beacon: new_beacon }.into());
        }

        let beacon_implementation =
            Erc1967Utils::get_beacon_implementation(context, new_beacon)?;

        if context.vm().code_size(beacon_implementation) == 0 {
            return Err(ERC1967InvalidImplementation {
                implementation: beacon_implementation,
            }
            .into());
        }

        StorageSlot::get_slot::<StorageAddress>(context, BEACON_SLOT)
            .set(new_beacon);

        Ok(beacon_implementation)
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
    fn get_beacon_implementation<T: TopLevelStorage + HostAccess<Host = VM>>(
        context: &T,
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

#[cfg(test)]
mod tests {
    #![allow(clippy::needless_pass_by_value, clippy::unused_self)]

    use alloy_sol_types::{SolCall, SolValue};
    use stylus_sdk::testing::TestVM;
    use stylus_sdk::{
        alloy_primitives::{Address, U256},
        alloy_sol_types::sol,
        function_selector,
        prelude::*,
    };

    use super::*;

    fn mock_beacon(vm: &TestVM, beacon: Address, implementation: Address) {
        vm.set_code(beacon, vec![1]);
        vm.mock_static_call(
            beacon,
            IBeaconMock::implementationCall {}.abi_encode(),
            Ok(implementation.abi_encode()),
        );
    }

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

    sol! {
        #[derive(Debug)]
        #[allow(missing_docs)]
        error ImplementationSolidityError();

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

    sol! {
        interface IImplementation {
            function emitOrError(bool should_error) external;
        }

        interface IBeaconMock {
            function implementation() external view returns (address);
        }
    }

    use crate::test_utils::assert_emitted;

    #[test]
    fn get_implementation_returns_zero_by_default() {
        let contract_vm = TestVM::default();
        let contract_address = Address::from([160u8; 20]);
        contract_vm.set_contract_address(contract_address);
        let mut contract = TestContract::from(&contract_vm);
        let alice = Address::from([1u8; 20]);

        let implementation = ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .test_get_implementation();
        assert_eq!(implementation, Address::ZERO);
    }

    #[test]
    fn upgrade_to_and_call_with_valid_implementation_and_empty_data() {
        let contract_vm = TestVM::default();
        let contract_address = Address::from([160u8; 20]);
        contract_vm.set_contract_address(contract_address);
        let mut contract = TestContract::from(&contract_vm);
        let implementation_address = Address::from([161u8; 20]);
        contract_vm.set_code(implementation_address, vec![1]);
        let alice = Address::from([1u8; 20]);

        ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .test_upgrade_to_and_call(implementation_address, vec![].into())
        .expect("should be able to upgrade to valid implementation");

        let implementation_addr = ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .test_get_implementation();
        assert_eq!(implementation_addr, implementation_address);

        // verify event was emitted.
        assert_emitted(
            &contract_vm,
            &erc1967::Upgraded { implementation: implementation_address },
        );
    }

    #[test]
    fn upgrade_to_and_call_with_valid_implementation_and_data() {
        let contract_vm = TestVM::default();
        let contract_address = Address::from([160u8; 20]);
        contract_vm.set_contract_address(contract_address);
        let mut contract = TestContract::from(&contract_vm);
        let implementation_address = Address::from([161u8; 20]);
        contract_vm.set_code(implementation_address, vec![1]);
        let alice = Address::from([1u8; 20]);

        let data = IImplementation::emitOrErrorCall { should_error: false }
            .abi_encode();
        contract_vm.mock_delegate_call(
            implementation_address,
            data.clone(),
            Ok(Vec::new()),
        );
        ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .test_upgrade_to_and_call(implementation_address, data.into())
        .expect("should be able to upgrade to valid implementation with data");

        let implementation_addr = ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .test_get_implementation();
        assert_eq!(implementation_addr, implementation_address);

        // verify event was emitted.
        assert_emitted(
            &contract_vm,
            &erc1967::Upgraded { implementation: implementation_address },
        );

        // TODO: this should assert that the event was emitted on `contract`
        // (as it's the one that delegate called the upgrade)
        // https://github.com/OpenZeppelin/stylus-test-helpers/issues/111
    }

    #[test]
    fn upgrade_to_and_call_with_invalid_implementation() {
        let contract_vm = TestVM::default();
        let contract_address = Address::from([160u8; 20]);
        contract_vm.set_contract_address(contract_address);
        let mut contract = TestContract::from(&contract_vm);
        let alice = Address::from([1u8; 20]);

        let invalid_address = alice; // address with no code.
        let err = ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .test_upgrade_to_and_call(invalid_address, vec![].into())
        .expect_err("should fail when upgrading to invalid implementation");

        assert_eq!(
            err,
            Error::InvalidImplementation(ERC1967InvalidImplementation {
                implementation: invalid_address,
            })
            .encode()
        );
        // implementation should remain unchanged.
        let implementation = ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .test_get_implementation();
        assert_eq!(implementation, Address::ZERO);
    }

    #[test]
    fn invalid_implementation_precedes_nonpayable_error() {
        let contract_vm = TestVM::default();
        let contract_address = Address::from([160u8; 20]);
        contract_vm.set_contract_address(contract_address);
        let mut contract = TestContract::from(&contract_vm);
        let invalid_address = Address::from([161u8; 20]);
        contract_vm.set_sender(Address::from([1u8; 20]));
        contract_vm.set_value(U256::from(1));

        let err = contract
            .test_upgrade_to_and_call(invalid_address, vec![].into())
            .expect_err("invalid implementation must be checked first");

        assert_eq!(
            err,
            Error::InvalidImplementation(ERC1967InvalidImplementation {
                implementation: invalid_address,
            })
            .encode()
        );
        assert_eq!(contract.test_get_implementation(), Address::ZERO);
        assert!(contract_vm.get_emitted_logs().is_empty());
    }

    #[test]
    fn upgrade_to_and_call_with_zero_implementation() {
        let contract_vm = TestVM::default();
        let contract_address = Address::from([160u8; 20]);
        contract_vm.set_contract_address(contract_address);
        let mut contract = TestContract::from(&contract_vm);
        let alice = Address::from([1u8; 20]);

        let err = ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .test_upgrade_to_and_call(Address::ZERO, vec![].into())
        .expect_err("should fail when upgrading to zero implementation");

        assert_eq!(
            err,
            Error::InvalidImplementation(ERC1967InvalidImplementation {
                implementation: Address::ZERO,
            })
            .encode()
        );
    }

    #[test]
    fn upgrade_to_and_call_with_same_implementation() {
        let contract_vm = TestVM::default();
        let contract_address = Address::from([160u8; 20]);
        contract_vm.set_contract_address(contract_address);
        let mut contract = TestContract::from(&contract_vm);
        let implementation_address = Address::from([161u8; 20]);
        contract_vm.set_code(implementation_address, vec![1]);
        let alice = Address::from([1u8; 20]);

        // first upgrade.
        ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .test_upgrade_to_and_call(implementation_address, vec![].into())
        .expect("should be able to upgrade first time");

        // upgrade to same implementation.
        ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .test_upgrade_to_and_call(implementation_address, vec![].into())
        .expect("should be able to upgrade to same implementation");

        let implementation_addr = ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .test_get_implementation();
        assert_eq!(implementation_addr, implementation_address);

        // event should still be emitted.
        assert_emitted(
            &contract_vm,
            &erc1967::Upgraded { implementation: implementation_address },
        );
    }

    #[test]
    fn upgrade_to_and_call_with_delegate_call_failure() {
        let contract_vm = TestVM::default();
        let contract_address = Address::from([160u8; 20]);
        contract_vm.set_contract_address(contract_address);
        let mut contract = TestContract::from(&contract_vm);
        let implementation_address = Address::from([161u8; 20]);
        contract_vm.set_code(implementation_address, vec![1]);
        let alice = Address::from([1u8; 20]);

        let data = function_selector!("nonExistentFunction").to_vec();
        let revert_reason = format!(
            "function not found for selector '{0}' and no fallback defined",
            u32::from_be_bytes(TryInto::try_into(data.clone()).unwrap())
        )
        .into_bytes();
        contract_vm.mock_delegate_call(
            implementation_address,
            data.clone(),
            Err(revert_reason.clone()),
        );

        let err = ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .test_upgrade_to_and_call(implementation_address, data.clone().into())
        .expect_err("should fail when delegate call fails");

        assert_eq!(
            err,
            Error::FailedCallWithReason(address::FailedCallWithReason {
                reason: stylus_sdk::stylus_core::calls::errors::Error::Revert(
                    revert_reason
                )
                .encode()
                .into()
            })
            .encode(),
        );

        assert_eq!(contract.test_get_implementation(), Address::ZERO);
        assert!(contract_vm.get_emitted_logs().is_empty());
    }

    #[test]
    fn upgrade_to_and_call_with_implementation_reverts() {
        let contract_vm = TestVM::default();
        let contract_address = Address::from([160u8; 20]);
        contract_vm.set_contract_address(contract_address);
        let mut contract = TestContract::from(&contract_vm);
        let implementation_address = Address::from([161u8; 20]);
        contract_vm.set_code(implementation_address, vec![1]);
        let alice = Address::from([1u8; 20]);

        let data = IImplementation::emitOrErrorCall { should_error: true }
            .abi_encode();
        contract_vm.mock_delegate_call(
            implementation_address,
            data.clone(),
            Err(ImplementationError::ImplementationError(
                ImplementationSolidityError {},
            )
            .encode()),
        );

        let err = ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .test_upgrade_to_and_call(implementation_address, data.clone().into())
        .expect_err("should fail when implementation reverts");

        assert_eq!(
            err,
            Error::FailedCallWithReason(address::FailedCallWithReason {
                reason: ImplementationError::ImplementationError(
                    ImplementationSolidityError {}
                )
                .encode()
                .into(),
            })
            .encode()
        );
    }

    #[test]
    fn upgrade_to_and_call_reverts_with_empty_data_with_value() {
        let contract_vm = TestVM::default();
        let contract_address = Address::from([160u8; 20]);
        contract_vm.set_contract_address(contract_address);
        let mut contract = TestContract::from(&contract_vm);
        let implementation_address = Address::from([161u8; 20]);
        contract_vm.set_code(implementation_address, vec![1]);
        let alice = Address::from([1u8; 20]);

        contract_vm.set_balance(alice, U256::from(1000));
        contract_vm.set_sender(alice);
        contract_vm.set_value(U256::from(1000));

        let err = contract
            .test_upgrade_to_and_call(implementation_address, vec![].into())
            .expect_err("should fail with ERC1967NonPayable");

        assert_eq!(err, Error::NonPayable(ERC1967NonPayable {}).encode());
        assert_eq!(contract.test_get_implementation(), Address::ZERO);
        assert!(contract_vm.get_emitted_logs().is_empty());
    }

    #[test]
    fn get_admin_returns_zero_by_default() {
        let contract_vm = TestVM::default();
        let contract_address = Address::from([160u8; 20]);
        contract_vm.set_contract_address(contract_address);
        let mut contract = TestContract::from(&contract_vm);
        let alice = Address::from([1u8; 20]);

        let admin = ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .test_get_admin();
        assert_eq!(admin, Address::ZERO);
    }

    #[test]
    fn change_admin_with_valid_address() {
        let contract_vm = TestVM::default();
        let contract_address = Address::from([160u8; 20]);
        contract_vm.set_contract_address(contract_address);
        let mut contract = TestContract::from(&contract_vm);
        let alice = Address::from([1u8; 20]);
        let bob = Address::from([2u8; 20]);

        ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .test_change_admin(bob)
        .expect("should be able to change admin to valid address");

        let admin = ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .test_get_admin();
        assert_eq!(admin, bob);

        // Verify event was emitted
        assert_emitted(
            &contract_vm,
            &erc1967::AdminChanged {
                previous_admin: Address::ZERO,
                new_admin: bob,
            },
        );

        // verify that changing admin to [`Address::ZERO`] fails.
        let err = ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .test_change_admin(Address::ZERO)
        .expect_err("should fail when changing admin to zero address");

        assert_eq!(
            err,
            Error::InvalidAdmin(ERC1967InvalidAdmin { admin: Address::ZERO })
                .encode()
        );
        assert_eq!(contract_vm.get_emitted_logs().len(), 1);

        // admin should remain unchanged.
        let admin = ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .test_get_admin();
        assert_eq!(admin, bob);
    }

    #[test]
    fn change_admin_multiple_times() {
        let contract_vm = TestVM::default();
        let contract_address = Address::from([160u8; 20]);
        contract_vm.set_contract_address(contract_address);
        let mut contract = TestContract::from(&contract_vm);
        let alice = Address::from([1u8; 20]);
        let bob = Address::from([2u8; 20]);
        let charlie = Address::from([3u8; 20]);

        // first change.
        ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .test_change_admin(bob)
        .expect("should be able to change admin first time");

        let admin = ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .test_get_admin();
        assert_eq!(admin, bob);

        // second change.
        ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .test_change_admin(charlie)
        .expect("should be able to change admin second time");

        let admin = ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .test_get_admin();
        assert_eq!(admin, charlie);

        // verify events were emitted.
        assert_emitted(
            &contract_vm,
            &erc1967::AdminChanged {
                previous_admin: Address::ZERO,
                new_admin: bob,
            },
        );
        assert_emitted(
            &contract_vm,
            &erc1967::AdminChanged { previous_admin: bob, new_admin: charlie },
        );
    }

    #[test]
    fn change_admin_with_same_address() {
        let contract_vm = TestVM::default();
        let contract_address = Address::from([160u8; 20]);
        contract_vm.set_contract_address(contract_address);
        let mut contract = TestContract::from(&contract_vm);
        let alice = Address::from([1u8; 20]);
        let bob = Address::from([2u8; 20]);

        // first change.
        ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .test_change_admin(bob)
        .expect("should be able to change admin first time");

        // change to same address.
        ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .test_change_admin(bob)
        .expect("should be able to change admin to same address");

        let admin = ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .test_get_admin();
        assert_eq!(admin, bob);

        // event should still be emitted.
        assert_emitted(
            &contract_vm,
            &erc1967::AdminChanged {
                previous_admin: Address::ZERO,
                new_admin: bob,
            },
        );
        assert_emitted(
            &contract_vm,
            &erc1967::AdminChanged { previous_admin: bob, new_admin: bob },
        );
    }

    #[test]
    fn get_beacon_returns_zero_by_default() {
        let contract_vm = TestVM::default();
        let contract_address = Address::from([160u8; 20]);
        contract_vm.set_contract_address(contract_address);
        let mut contract = TestContract::from(&contract_vm);
        let alice = Address::from([1u8; 20]);

        let beacon = ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .test_get_beacon();
        assert_eq!(beacon, Address::ZERO);
    }

    #[test]
    fn upgrade_beacon_to_and_call_with_valid_beacon_and_empty_data() {
        let contract_vm = TestVM::default();
        let contract_address = Address::from([160u8; 20]);
        contract_vm.set_contract_address(contract_address);
        let mut contract = TestContract::from(&contract_vm);
        let beacon_test_address = Address::from([161u8; 20]);
        let implementation_address = Address::from([162u8; 20]);
        contract_vm.set_code(implementation_address, vec![1]);
        let alice = Address::from([1u8; 20]);

        mock_beacon(&contract_vm, beacon_test_address, implementation_address);

        ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .test_upgrade_beacon_to_and_call(beacon_test_address, vec![].into())
        .expect("should be able to upgrade to valid beacon");

        let actual_beacon_address = ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .test_get_beacon();
        assert_eq!(actual_beacon_address, beacon_test_address);

        // verify event was emitted.
        assert_emitted(
            &contract_vm,
            &erc1967::BeaconUpgraded { beacon: beacon_test_address },
        );
    }

    #[test]
    fn upgrade_beacon_to_and_call_with_valid_beacon_and_data() {
        let contract_vm = TestVM::default();
        let contract_address = Address::from([160u8; 20]);
        contract_vm.set_contract_address(contract_address);
        let mut contract = TestContract::from(&contract_vm);
        let beacon_test_address = Address::from([161u8; 20]);
        let implementation_address = Address::from([162u8; 20]);
        contract_vm.set_code(implementation_address, vec![1]);
        let alice = Address::from([1u8; 20]);

        mock_beacon(&contract_vm, beacon_test_address, implementation_address);

        let data = IImplementation::emitOrErrorCall { should_error: false }
            .abi_encode();
        contract_vm.mock_delegate_call(
            implementation_address,
            data.clone(),
            Ok(Vec::new()),
        );
        ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .test_upgrade_beacon_to_and_call(beacon_test_address, data.into())
        .expect("should be able to upgrade to valid beacon with data");

        let actual_beacon_address = ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .test_get_beacon();
        assert_eq!(actual_beacon_address, beacon_test_address);

        // verify event was emitted.
        assert_emitted(
            &contract_vm,
            &erc1967::BeaconUpgraded { beacon: beacon_test_address },
        );

        // TODO: this should assert that the event was emitted on `contract`
        // (as it's the one that delegate called the upgrade)
        // https://github.com/OpenZeppelin/stylus-test-helpers/issues/111
    }

    #[test]
    fn upgrade_beacon_to_and_call_with_invalid_beacon() {
        let contract_vm = TestVM::default();
        let contract_address = Address::from([160u8; 20]);
        contract_vm.set_contract_address(contract_address);
        let mut contract = TestContract::from(&contract_vm);
        let alice = Address::from([1u8; 20]);

        let invalid_address = alice; // address with no code.
        let err = ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .test_upgrade_beacon_to_and_call(invalid_address, vec![].into())
        .expect_err("should fail when upgrading to invalid beacon");

        assert_eq!(
            err,
            Error::InvalidBeacon(ERC1967InvalidBeacon {
                beacon: invalid_address,
            })
            .encode()
        );

        // beacon should remain unchanged.
        let beacon = ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .test_get_beacon();
        assert_eq!(beacon, Address::ZERO);
    }

    #[test]
    fn invalid_beacon_precedes_nonpayable_error() {
        let contract_vm = TestVM::default();
        let contract_address = Address::from([160u8; 20]);
        contract_vm.set_contract_address(contract_address);
        let mut contract = TestContract::from(&contract_vm);
        let invalid_address = Address::from([161u8; 20]);
        contract_vm.set_sender(Address::from([1u8; 20]));
        contract_vm.set_value(U256::from(1));

        let err = contract
            .test_upgrade_beacon_to_and_call(invalid_address, vec![].into())
            .expect_err("invalid beacon must be checked first");

        assert_eq!(
            err,
            Error::InvalidBeacon(ERC1967InvalidBeacon {
                beacon: invalid_address,
            })
            .encode()
        );
        assert_eq!(contract.test_get_beacon(), Address::ZERO);
        assert!(contract_vm.get_emitted_logs().is_empty());
    }

    #[test]
    fn upgrade_beacon_to_and_call_with_zero_beacon() {
        let contract_vm = TestVM::default();
        let contract_address = Address::from([160u8; 20]);
        contract_vm.set_contract_address(contract_address);
        let mut contract = TestContract::from(&contract_vm);
        let beacon_address = Address::from([161u8; 20]);
        let implementation_address = Address::from([162u8; 20]);
        contract_vm.set_code(implementation_address, vec![1]);
        let alice = Address::from([1u8; 20]);

        mock_beacon(&contract_vm, beacon_address, implementation_address);

        ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .test_upgrade_beacon_to_and_call(beacon_address, vec![].into())
        .expect("should be able to upgrade to valid beacon");

        let err = ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .test_upgrade_beacon_to_and_call(Address::ZERO, vec![].into())
        .expect_err("should fail when upgrading to zero beacon");

        assert_eq!(
            err,
            Error::InvalidBeacon(ERC1967InvalidBeacon {
                beacon: Address::ZERO,
            })
            .encode()
        );
    }

    #[test]
    fn upgrade_beacon_to_and_call_with_beacon_returning_invalid_implementation()
    {
        let contract_vm = TestVM::default();
        let contract_address = Address::from([160u8; 20]);
        contract_vm.set_contract_address(contract_address);
        let mut contract = TestContract::from(&contract_vm);
        let beacon_test_address = Address::from([161u8; 20]);
        let alice = Address::from([1u8; 20]);

        // beacon returns an address with no code.
        let invalid_address = alice; // address with no code.
        mock_beacon(&contract_vm, beacon_test_address, invalid_address);

        let err = ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .test_upgrade_beacon_to_and_call(beacon_test_address, vec![].into())
        .expect_err("should fail when beacon returns invalid implementation");

        assert_eq!(
            err,
            Error::InvalidImplementation(ERC1967InvalidImplementation {
                implementation: invalid_address,
            })
            .encode()
        );
        assert_eq!(contract.test_get_beacon(), Address::ZERO);
    }

    #[test]
    fn upgrade_beacon_to_and_call_with_beacon_returning_zero_implementation() {
        let contract_vm = TestVM::default();
        let contract_address = Address::from([160u8; 20]);
        contract_vm.set_contract_address(contract_address);
        let mut contract = TestContract::from(&contract_vm);
        let beacon_address = Address::from([161u8; 20]);
        let alice = Address::from([1u8; 20]);

        // beacon returns [`Address::ZERO`].
        mock_beacon(&contract_vm, beacon_address, Address::ZERO);

        let err = ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .test_upgrade_beacon_to_and_call(beacon_address, vec![].into())
        .expect_err("should fail when beacon returns zero implementation");

        assert_eq!(
            err,
            Error::InvalidImplementation(ERC1967InvalidImplementation {
                implementation: Address::ZERO,
            })
            .encode()
        );
        assert_eq!(contract.test_get_beacon(), Address::ZERO);
    }

    #[test]
    fn upgrade_beacon_to_and_call_multiple_times() {
        let contract_vm = TestVM::default();
        let contract_address = Address::from([160u8; 20]);
        contract_vm.set_contract_address(contract_address);
        let mut contract = TestContract::from(&contract_vm);
        let beacon1_address = Address::from([161u8; 20]);
        let beacon2_address = Address::from([162u8; 20]);
        let implementation_address = Address::from([163u8; 20]);
        contract_vm.set_code(implementation_address, vec![1]);
        let alice = Address::from([1u8; 20]);

        mock_beacon(&contract_vm, beacon1_address, implementation_address);
        mock_beacon(&contract_vm, beacon2_address, implementation_address);

        // first upgrade.
        ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .test_upgrade_beacon_to_and_call(beacon1_address, vec![].into())
        .expect("should be able to upgrade beacon first time");

        let beacon = ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .test_get_beacon();
        assert_eq!(beacon, beacon1_address);

        // second upgrade.
        ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .test_upgrade_beacon_to_and_call(beacon2_address, vec![].into())
        .expect("should be able to upgrade beacon second time");

        let beacon = ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .test_get_beacon();
        assert_eq!(beacon, beacon2_address);

        // verify events were emitted.
        assert_emitted(
            &contract_vm,
            &erc1967::BeaconUpgraded { beacon: beacon1_address },
        );
        assert_emitted(
            &contract_vm,
            &erc1967::BeaconUpgraded { beacon: beacon2_address },
        );
    }

    #[test]
    fn upgrade_beacon_to_and_call_with_same_beacon() {
        let contract_vm = TestVM::default();
        let contract_address = Address::from([160u8; 20]);
        contract_vm.set_contract_address(contract_address);
        let mut contract = TestContract::from(&contract_vm);
        let beacon_test_address = Address::from([161u8; 20]);
        let implementation_address = Address::from([162u8; 20]);
        contract_vm.set_code(implementation_address, vec![1]);
        let alice = Address::from([1u8; 20]);

        mock_beacon(&contract_vm, beacon_test_address, implementation_address);

        // first upgrade.
        ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .test_upgrade_beacon_to_and_call(beacon_test_address, vec![].into())
        .expect("should be able to upgrade beacon first time");

        // upgrade to same beacon.
        ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .test_upgrade_beacon_to_and_call(beacon_test_address, vec![].into())
        .expect("should be able to upgrade to same beacon");

        let actual_beacon_address = ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .test_get_beacon();
        assert_eq!(actual_beacon_address, beacon_test_address);

        // event should still be emitted.
        assert_emitted(
            &contract_vm,
            &erc1967::BeaconUpgraded { beacon: beacon_test_address },
        );
    }

    #[test]
    fn upgrade_beacon_to_and_call_with_delegate_call_failure() {
        let contract_vm = TestVM::default();
        let contract_address = Address::from([160u8; 20]);
        contract_vm.set_contract_address(contract_address);
        let mut contract = TestContract::from(&contract_vm);
        let beacon_address = Address::from([161u8; 20]);
        let implementation_address = Address::from([162u8; 20]);
        contract_vm.set_code(implementation_address, vec![1]);
        let alice = Address::from([1u8; 20]);

        mock_beacon(&contract_vm, beacon_address, implementation_address);

        let data = function_selector!("nonExistentFunction").to_vec();
        let revert_reason = format!(
            "function not found for selector '{0}' and no fallback defined",
            u32::from_be_bytes(TryInto::try_into(data.clone()).unwrap())
        )
        .into_bytes();
        contract_vm.mock_delegate_call(
            implementation_address,
            data.clone(),
            Err(revert_reason.clone()),
        );

        let err = ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .test_upgrade_beacon_to_and_call(beacon_address, data.clone().into())
        .expect_err("should fail when delegate call fails");

        assert_eq!(
            err,
            Error::FailedCallWithReason(address::FailedCallWithReason {
                reason: stylus_sdk::stylus_core::calls::errors::Error::Revert(
                    revert_reason
                )
                .encode()
                .into()
            })
            .encode(),
        );

        assert_eq!(contract.test_get_beacon(), Address::ZERO);
        assert!(contract_vm.get_emitted_logs().is_empty());
    }

    #[test]
    fn upgrade_beacon_to_and_call_with_beacon_implementation_reverts() {
        let contract_vm = TestVM::default();
        let contract_address = Address::from([160u8; 20]);
        contract_vm.set_contract_address(contract_address);
        let mut contract = TestContract::from(&contract_vm);
        let beacon_address = Address::from([161u8; 20]);
        let implementation_address = Address::from([162u8; 20]);
        contract_vm.set_code(implementation_address, vec![1]);
        let alice = Address::from([1u8; 20]);

        mock_beacon(&contract_vm, beacon_address, implementation_address);

        let data = IImplementation::emitOrErrorCall { should_error: true }
            .abi_encode();
        contract_vm.mock_delegate_call(
            implementation_address,
            data.clone(),
            Err(ImplementationError::ImplementationError(
                ImplementationSolidityError {},
            )
            .encode()),
        );

        let err = ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .test_upgrade_beacon_to_and_call(beacon_address, data.clone().into())
        .expect_err("should fail when beacon implementation reverts");

        assert_eq!(
            err,
            Error::FailedCallWithReason(address::FailedCallWithReason {
                reason: ImplementationError::ImplementationError(
                    ImplementationSolidityError {}
                )
                .encode()
                .into(),
            })
            .encode()
        );
    }

    #[test]
    fn upgrade_beacon_to_and_call_reverts_with_empty_data_with_value() {
        let contract_vm = TestVM::default();
        let contract_address = Address::from([160u8; 20]);
        contract_vm.set_contract_address(contract_address);
        let mut contract = TestContract::from(&contract_vm);
        let beacon_address = Address::from([161u8; 20]);
        let implementation_address = Address::from([162u8; 20]);
        contract_vm.set_code(implementation_address, vec![1]);
        let alice = Address::from([1u8; 20]);

        mock_beacon(&contract_vm, beacon_address, implementation_address);

        contract_vm.set_balance(alice, U256::from(1000));
        contract_vm.set_sender(alice);
        contract_vm.set_value(U256::from(1000));

        let err = contract
            .test_upgrade_beacon_to_and_call(beacon_address, vec![].into())
            .expect_err("should fail with ERC1967NonPayable");

        assert_eq!(err, Error::NonPayable(ERC1967NonPayable {}).encode());
        assert_eq!(contract.test_get_beacon(), Address::ZERO);
        assert!(contract_vm.get_emitted_logs().is_empty());
    }

    // test storage slot isolation.
    #[test]
    fn storage_slots_are_independent() {
        let contract_vm = TestVM::default();
        let contract_address = Address::from([160u8; 20]);
        contract_vm.set_contract_address(contract_address);
        let mut contract = TestContract::from(&contract_vm);
        let implementation_address = Address::from([161u8; 20]);
        contract_vm.set_code(implementation_address, vec![1]);
        let beacon_test_address = Address::from([162u8; 20]);
        let alice = Address::from([1u8; 20]);
        let bob = Address::from([2u8; 20]);

        mock_beacon(&contract_vm, beacon_test_address, implementation_address);

        // set implementation.
        ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .test_upgrade_to_and_call(implementation_address, vec![].into())
        .expect("should be able to set implementation");

        // set admin.
        ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .test_change_admin(bob)
        .expect("should be able to set admin");

        // set beacon.
        ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .test_upgrade_beacon_to_and_call(beacon_test_address, vec![].into())
        .expect("should be able to set beacon");

        // verify all are set correctly.
        let implementation_addr = ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .test_get_implementation();
        let admin = ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .test_get_admin();
        let actual_beacon_address = ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .test_get_beacon();

        assert_eq!(implementation_addr, implementation_address);
        assert_eq!(admin, bob);
        assert_eq!(actual_beacon_address, beacon_test_address);
    }
}
