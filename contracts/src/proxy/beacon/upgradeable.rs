//! Implementation of the [`UpgradeableBeacon`] contract.

use alloc::{vec, vec::Vec};

use alloy_primitives::Address;
pub use sol::*;
use stylus_sdk::{
    prelude::*, storage::StorageAddress,
    stylus_core::calls::errors::MethodError,
};

use crate::{
    access::ownable::{self, IOwnable, Ownable},
    proxy::beacon::IBeacon,
    utils::address::AddressUtils,
};

#[cfg_attr(coverage_nightly, coverage(off))]
mod sol {
    use alloy_sol_macro::sol;

    sol! {
        /// Indicates an error related to the fact that the `implementation`
        /// of the beacon is invalid.
        ///
        /// * `implementation` - Address of the invalid implementation.
        #[derive(Debug)]
        #[allow(missing_docs)]
        error BeaconInvalidImplementation(address implementation);

        /// Emitted when the implementation returned by the beacon is changed.
        #[derive(Debug)]
        #[allow(missing_docs)]
        event Upgraded(address indexed implementation);
    }
}

/// An [`UpgradeableBeacon`] error.
#[derive(SolidityError, Debug)]
pub enum Error {
    /// Indicates an error related to the fact that the `implementation`
    /// of the beacon is invalid.
    InvalidImplementation(BeaconInvalidImplementation),
    /// The caller account is not authorized to perform an operation.
    UnauthorizedAccount(ownable::OwnableUnauthorizedAccount),
    /// Indicates an error related to the fact that the `owner` of the
    /// beacon is invalid.
    InvalidOwner(ownable::OwnableInvalidOwner),
}

impl MethodError for Error {
    fn encode(self) -> alloc::vec::Vec<u8> {
        self.into()
    }
}

impl From<ownable::Error> for Error {
    fn from(err: ownable::Error) -> Self {
        match err {
            ownable::Error::UnauthorizedAccount(err) => {
                Error::UnauthorizedAccount(err)
            }
            ownable::Error::InvalidOwner(err) => Error::InvalidOwner(err),
        }
    }
}

/// This contract is used in conjunction with one or more instances of
/// [`BeaconProxy`][BeaconProxy] to determine their implementation contract,
/// which is where they will delegate all function calls.
///
/// An owner is able to change the implementation the beacon points to, thus
/// upgrading the proxies that use this beacon.
///
/// [BeaconProxy]: super::BeaconProxy
#[public]
// The `#[public]` macro adds an undocumented helper method to the trait.
#[allow(missing_docs)]
pub trait IUpgradeableBeacon: IBeacon + IOwnable {
    /// Upgrades the beacon to a new implementation.
    ///
    /// # Arguments
    ///
    /// * `&mut self` - Write access to the contract's state.
    /// * `new_implementation` - The address of the new implementation.
    ///
    /// # Errors
    ///
    /// Implementing contracts should define their own error types for this
    /// function. Typically, errors may include:
    /// * The caller is not authorized to perform the upgrade.
    /// * The new implementation address is invalid (e.g., not a contract).
    /// * The upgrade operation failed for contract-specific reasons.
    ///
    /// The error should be encoded as a [`Vec<u8>`].
    fn upgrade_to(
        &mut self,
        new_implementation: Address,
    ) -> Result<(), Vec<u8>>;
}

/// State of an [`UpgradeableBeacon`] contract.
#[storage]
pub struct UpgradeableBeacon {
    /// The address of the implementation contract.
    implementation: StorageAddress,
    /// The [`Ownable`] contract that owns the beacon.
    ownable: Ownable,
}

unsafe impl TopLevelStorage for UpgradeableBeacon {}

#[public]
#[implements(IBeacon, IOwnable)]
impl UpgradeableBeacon {
    /// Sets the address of the initial implementation, and the initial owner
    /// who can upgrade the beacon.
    ///
    /// # Arguments
    ///
    /// * `&mut self` - Write access to the contract's state.
    /// * `implementation` - The address of the initial implementation.
    ///
    /// # Errors
    ///
    /// * [`Error::InvalidImplementation`] - If `implementation` is not a
    ///   contract.
    /// * [`Error::UnauthorizedAccount`] - If the caller is not the owner.
    ///
    /// # Events
    ///
    /// * [`Upgraded`].
    #[constructor]
    pub fn constructor(
        &mut self,
        implementation: Address,
        initial_owner: Address,
    ) -> Result<(), Error> {
        self.ownable.constructor(initial_owner)?;
        self.set_implementation(implementation)?;
        Ok(())
    }

    /// Upgrades the beacon to a new implementation.
    ///
    /// # Arguments
    ///
    /// * `&mut self` - Write access to the contract's state.
    /// * `new_implementation` - The address of the new implementation.
    ///
    /// # Errors
    ///
    /// * [`Error::InvalidImplementation`] - If `new_implementation` is not a
    ///   contract.
    /// * [`Error::UnauthorizedAccount`] - If the caller is not the owner.
    ///
    /// # Events
    ///
    /// * [`Upgraded`].
    pub fn upgrade_to(
        &mut self,
        new_implementation: Address,
    ) -> Result<(), Error> {
        self.ownable.only_owner()?;
        self.set_implementation(new_implementation)
    }

    /// Upgrades the beacon to a new implementation.
    ///
    /// # Arguments
    ///
    /// * `&mut self` - Write access to the contract's state.
    /// * `new_implementation` - The address of the new implementation.
    ///
    /// # Errors
    ///
    /// * [`Error::InvalidImplementation`] - If `new_implementation` is not a
    ///   contract.
    ///
    /// # Events
    ///
    /// * [`Upgraded`].
    fn set_implementation(
        &mut self,
        new_implementation: Address,
    ) -> Result<(), Error> {
        if !AddressUtils::has_code(self, new_implementation) {
            return Err(Error::InvalidImplementation(
                BeaconInvalidImplementation {
                    implementation: new_implementation,
                },
            ));
        }
        self.implementation.set(new_implementation);
        self.vm().log(Upgraded { implementation: new_implementation });
        Ok(())
    }
}

#[public]
impl IBeacon for UpgradeableBeacon {
    fn implementation(&self) -> Result<Address, Vec<u8>> {
        Ok(self.implementation.get())
    }
}

#[public]
impl IOwnable for UpgradeableBeacon {
    fn owner(&self) -> Address {
        self.ownable.owner()
    }

    fn transfer_ownership(
        &mut self,
        new_owner: Address,
    ) -> Result<(), Vec<u8>> {
        Ok(self.ownable.transfer_ownership(new_owner)?)
    }

    fn renounce_ownership(&mut self) -> Result<(), Vec<u8>> {
        Ok(self.ownable.renounce_ownership()?)
    }
}

// Not built with `export-abi`, see `proxy::test_helpers`.
#[cfg(all(test, not(feature = "export-abi")))]
mod tests {
    use stylus_sdk::{
        alloy_primitives::{address, Address},
        testing::TestVM,
    };

    use super::*;
    use crate::{
        proxy::{
            beacon::IBeacon,
            test_helpers::{assert_emitted, deploy},
        },
        test_utils::{ALICE, BOB, CHARLIE},
    };

    /// Contracts the beacon can point to.
    const IMPLEMENTATION: Address =
        address!("0x000000000000000000000000000000000000e200");
    const IMPLEMENTATION_2: Address =
        address!("0x000000000000000000000000000000000000e201");
    const IMPLEMENTATION_3: Address =
        address!("0x000000000000000000000000000000000000e202");

    #[test]
    fn constructor() {
        let vm = TestVM::default();
        let mut beacon = UpgradeableBeacon::from(&vm);
        vm.set_sender(ALICE);
        deploy(&vm, IMPLEMENTATION);

        beacon.constructor(IMPLEMENTATION, ALICE).unwrap();

        let implementation = beacon
            .implementation()
            .expect("should be able to get implementation");
        assert_eq!(implementation, IMPLEMENTATION);

        let owner = beacon.owner();
        assert_eq!(owner, ALICE);
    }

    #[test]
    fn constructor_with_invalid_implementation() {
        let vm = TestVM::default();
        let mut beacon = UpgradeableBeacon::from(&vm);
        vm.set_sender(ALICE);

        let invalid_address = ALICE;
        let err = beacon.constructor(invalid_address, ALICE).expect_err(
            "should fail when constructor has invalid implementation",
        );

        assert!(matches!(
            err,
            Error::InvalidImplementation(BeaconInvalidImplementation {
                implementation
            }) if implementation == invalid_address
        ));
    }

    #[test]
    fn constructor_with_zero_owner() {
        let vm = TestVM::default();
        let mut beacon = UpgradeableBeacon::from(&vm);
        vm.set_sender(ALICE);
        deploy(&vm, IMPLEMENTATION);

        let err = beacon
            .constructor(IMPLEMENTATION, Address::ZERO)
            .expect_err("should fail when constructor has zero owner");

        assert!(matches!(
            err,
            Error::InvalidOwner(ownable::OwnableInvalidOwner {
                owner
            }) if owner == Address::ZERO
        ));
    }

    #[test]
    fn constructor_with_zero_implementation() {
        let vm = TestVM::default();
        let mut beacon = UpgradeableBeacon::from(&vm);
        vm.set_sender(ALICE);

        let err = beacon
            .constructor(Address::ZERO, ALICE)
            .expect_err("should fail when constructor has zero implementation");

        assert!(matches!(
            err,
            Error::InvalidImplementation(BeaconInvalidImplementation {
                implementation
            }) if implementation == Address::ZERO
        ));
    }

    #[test]
    fn upgrade_to_valid_implementation() {
        let vm = TestVM::default();
        let mut beacon = UpgradeableBeacon::from(&vm);
        vm.set_sender(ALICE);
        deploy(&vm, IMPLEMENTATION);
        deploy(&vm, IMPLEMENTATION_2);

        beacon.constructor(IMPLEMENTATION, ALICE).unwrap();

        // upgrade to new implementation.
        beacon
            .upgrade_to(IMPLEMENTATION_2)
            .expect("should be able to upgrade to valid implementation");

        let implementation = beacon
            .implementation()
            .expect("should be able to get implementation");
        assert_eq!(implementation, IMPLEMENTATION_2);

        // verify event was emitted.
        assert_emitted(&vm, &Upgraded { implementation: IMPLEMENTATION_2 });
    }

    #[test]
    fn upgrade_to_invalid_implementation() {
        let vm = TestVM::default();
        let mut beacon = UpgradeableBeacon::from(&vm);
        vm.set_sender(ALICE);
        deploy(&vm, IMPLEMENTATION);

        beacon.constructor(IMPLEMENTATION, ALICE).unwrap();

        // try to upgrade to address with no code.
        let invalid_address = ALICE;
        let err = beacon
            .upgrade_to(invalid_address)
            .expect_err("should fail when upgrading to invalid implementation");

        assert!(matches!(
            err,
            Error::InvalidImplementation(BeaconInvalidImplementation {
                implementation
            }) if implementation == invalid_address
        ));

        // implementation should remain unchanged.
        let implementation = beacon
            .implementation()
            .expect("should be able to get implementation");
        assert_eq!(implementation, IMPLEMENTATION);
    }

    #[test]
    fn upgrade_to_unauthorized() {
        let vm = TestVM::default();
        let mut beacon = UpgradeableBeacon::from(&vm);
        vm.set_sender(ALICE);
        deploy(&vm, IMPLEMENTATION);
        deploy(&vm, IMPLEMENTATION_2);

        beacon.constructor(IMPLEMENTATION, ALICE).unwrap();

        // try to upgrade from non-owner account.
        vm.set_sender(BOB);
        let err = beacon
            .upgrade_to(IMPLEMENTATION_2)
            .expect_err("should fail when called by non-owner");

        assert!(matches!(
            err,
            Error::UnauthorizedAccount(ownable::OwnableUnauthorizedAccount {
                account
            }) if account == BOB
        ));

        // implementation should remain unchanged.
        vm.set_sender(ALICE);
        let implementation = beacon
            .implementation()
            .expect("should be able to get implementation");
        assert_eq!(implementation, IMPLEMENTATION);
    }

    #[test]
    fn upgrade_to_same_implementation() {
        let vm = TestVM::default();
        let mut beacon = UpgradeableBeacon::from(&vm);
        vm.set_sender(ALICE);
        deploy(&vm, IMPLEMENTATION);

        beacon.constructor(IMPLEMENTATION, ALICE).unwrap();

        // upgrade to the same implementation.
        beacon
            .upgrade_to(IMPLEMENTATION)
            .expect("should be able to upgrade to same implementation");

        // event should still be emitted.
        assert_emitted(&vm, &Upgraded { implementation: IMPLEMENTATION });

        let implementation = beacon
            .implementation()
            .expect("should be able to get implementation");
        assert_eq!(implementation, IMPLEMENTATION);
    }

    #[test]
    fn upgrade_to_zero_address() {
        let vm = TestVM::default();
        let mut beacon = UpgradeableBeacon::from(&vm);
        vm.set_sender(ALICE);
        deploy(&vm, IMPLEMENTATION);

        beacon.constructor(IMPLEMENTATION, ALICE).unwrap();

        // try to upgrade to [`Address::ZERO`].
        let err = beacon
            .upgrade_to(Address::ZERO)
            .expect_err("should fail when upgrading to zero address");

        assert!(matches!(
            err,
            Error::InvalidImplementation(BeaconInvalidImplementation {
                implementation
            }) if implementation == Address::ZERO
        ));

        // implementation should remain unchanged.
        let implementation = beacon
            .implementation()
            .expect("should be able to get implementation");
        assert_eq!(implementation, IMPLEMENTATION);
    }

    #[test]
    fn multiple_upgrades_emit_events() {
        let vm = TestVM::default();
        let mut beacon = UpgradeableBeacon::from(&vm);
        vm.set_sender(ALICE);
        deploy(&vm, IMPLEMENTATION);
        deploy(&vm, IMPLEMENTATION_2);
        deploy(&vm, IMPLEMENTATION_3);

        beacon.constructor(IMPLEMENTATION, ALICE).unwrap();

        // first upgrade.
        beacon
            .upgrade_to(IMPLEMENTATION_2)
            .expect("should be able to upgrade first time");

        assert_emitted(&vm, &Upgraded { implementation: IMPLEMENTATION_2 });

        // second upgrade.
        beacon
            .upgrade_to(IMPLEMENTATION_3)
            .expect("should be able to upgrade second time");

        assert_emitted(&vm, &Upgraded { implementation: IMPLEMENTATION_3 });

        let implementation = beacon
            .implementation()
            .expect("should be able to get implementation");
        assert_eq!(implementation, IMPLEMENTATION_3);
    }

    #[test]
    fn transfer_ownership() {
        let vm = TestVM::default();
        let mut beacon = UpgradeableBeacon::from(&vm);
        vm.set_sender(ALICE);
        deploy(&vm, IMPLEMENTATION);
        deploy(&vm, IMPLEMENTATION_2);

        beacon.constructor(IMPLEMENTATION, ALICE).unwrap();

        // transfer ownership to bob.
        beacon
            .transfer_ownership(BOB)
            .expect("should be able to transfer ownership");

        let owner = beacon.owner();
        assert_eq!(owner, BOB);

        // bob should now be able to upgrade.
        vm.set_sender(BOB);
        beacon
            .upgrade_to(IMPLEMENTATION_2)
            .expect("new owner should be able to upgrade");

        // alice should not be able to upgrade.
        vm.set_sender(ALICE);
        let err = beacon
            .upgrade_to(IMPLEMENTATION)
            .expect_err("should fail when called by non-owner");

        assert!(matches!(
            err,
            Error::UnauthorizedAccount(ownable::OwnableUnauthorizedAccount {
                account
            }) if account == ALICE
        ));
    }

    #[test]
    fn transfer_ownership_to_zero_address() {
        let vm = TestVM::default();
        let mut beacon = UpgradeableBeacon::from(&vm);
        vm.set_sender(ALICE);
        deploy(&vm, IMPLEMENTATION);

        beacon.constructor(IMPLEMENTATION, ALICE).unwrap();

        // try to transfer ownership to [`Address::ZERO`].
        let err = beacon.transfer_ownership(Address::ZERO).expect_err(
            "should fail when transferring ownership to zero address",
        );

        // the error should be from the ownable contract.
        assert_eq!(
            err,
            Error::InvalidOwner(ownable::OwnableInvalidOwner {
                owner: Address::ZERO
            })
            .encode()
        );

        // ownership should remain unchanged.
        let owner = beacon.owner();
        assert_eq!(owner, ALICE);
    }

    #[test]
    fn transfer_ownership_unauthorized() {
        let vm = TestVM::default();
        let mut beacon = UpgradeableBeacon::from(&vm);
        vm.set_sender(ALICE);
        deploy(&vm, IMPLEMENTATION);

        beacon.constructor(IMPLEMENTATION, ALICE).unwrap();

        // try to transfer ownership from non-owner account.
        vm.set_sender(BOB);
        let err = beacon
            .transfer_ownership(CHARLIE)
            .expect_err("should fail when called by non-owner");

        assert_eq!(
            err,
            Error::UnauthorizedAccount(ownable::OwnableUnauthorizedAccount {
                account: BOB
            })
            .encode()
        );

        // ownership should remain unchanged.
        vm.set_sender(ALICE);
        let owner = beacon.owner();
        assert_eq!(owner, ALICE);
    }

    #[test]
    fn renounce_ownership() {
        let vm = TestVM::default();
        let mut beacon = UpgradeableBeacon::from(&vm);
        vm.set_sender(ALICE);
        deploy(&vm, IMPLEMENTATION);
        deploy(&vm, IMPLEMENTATION_2);

        beacon.constructor(IMPLEMENTATION, ALICE).unwrap();

        // renounce ownership.
        beacon
            .renounce_ownership()
            .expect("should be able to renounce ownership");

        let owner = beacon.owner();
        assert_eq!(owner, Address::ZERO);

        // no one should be able to upgrade now.
        let err = beacon
            .upgrade_to(IMPLEMENTATION_2)
            .expect_err("should fail when no owner exists");

        assert!(matches!(
            err,
            Error::UnauthorizedAccount(ownable::OwnableUnauthorizedAccount {
                account
            }) if account == ALICE
        ));
    }

    #[test]
    fn renounce_ownership_unauthorized() {
        let vm = TestVM::default();
        let mut beacon = UpgradeableBeacon::from(&vm);
        vm.set_sender(ALICE);
        deploy(&vm, IMPLEMENTATION);

        beacon.constructor(IMPLEMENTATION, ALICE).unwrap();

        // try to renounce ownership from non-owner account.
        vm.set_sender(BOB);
        let err = beacon
            .renounce_ownership()
            .expect_err("should fail when called by non-owner");

        assert_eq!(
            err,
            Error::UnauthorizedAccount(ownable::OwnableUnauthorizedAccount {
                account: BOB
            })
            .encode()
        );

        // ownership should remain unchanged.
        vm.set_sender(ALICE);
        let owner = beacon.owner();
        assert_eq!(owner, ALICE);
    }

    #[test]
    fn upgrade_after_ownership_transfer_chain() {
        let vm = TestVM::default();
        let mut beacon = UpgradeableBeacon::from(&vm);
        vm.set_sender(ALICE);
        deploy(&vm, IMPLEMENTATION);
        deploy(&vm, IMPLEMENTATION_2);
        deploy(&vm, IMPLEMENTATION_3);

        beacon.constructor(IMPLEMENTATION, ALICE).unwrap();

        // alice transfers ownership to bob.
        beacon
            .transfer_ownership(BOB)
            .expect("should be able to transfer ownership to bob");

        // bob transfers ownership to charlie.
        vm.set_sender(BOB);
        beacon
            .transfer_ownership(CHARLIE)
            .expect("should be able to transfer ownership to charlie");

        // charlie should be able to upgrade.
        vm.set_sender(CHARLIE);
        beacon
            .upgrade_to(IMPLEMENTATION_2)
            .expect("charlie should be able to upgrade");

        vm.set_sender(ALICE);
        let implementation = beacon
            .implementation()
            .expect("should be able to get implementation");
        assert_eq!(implementation, IMPLEMENTATION_2);

        // alice and bob should not be able to upgrade anymore.
        let err = beacon
            .upgrade_to(IMPLEMENTATION_3)
            .expect_err("alice should not be able to upgrade");
        assert!(matches!(
            err,
            Error::UnauthorizedAccount(ownable::OwnableUnauthorizedAccount {
                account
            }) if account == ALICE
        ));

        vm.set_sender(BOB);
        let err = beacon
            .upgrade_to(IMPLEMENTATION_3)
            .expect_err("bob should not be able to upgrade");
        assert!(matches!(
            err,
            Error::UnauthorizedAccount(ownable::OwnableUnauthorizedAccount {
                account
            }) if account == BOB
        ));
    }

    #[test]
    fn upgrade_after_renounce_and_transfer() {
        let vm = TestVM::default();
        let mut beacon = UpgradeableBeacon::from(&vm);
        vm.set_sender(ALICE);
        deploy(&vm, IMPLEMENTATION);
        deploy(&vm, IMPLEMENTATION_2);

        beacon.constructor(IMPLEMENTATION, ALICE).unwrap();

        // alice renounces ownership.
        beacon
            .renounce_ownership()
            .expect("should be able to renounce ownership");

        // no one should be able to upgrade.
        let err = beacon
            .upgrade_to(IMPLEMENTATION_2)
            .expect_err("should fail when no owner exists");
        assert!(matches!(
            err,
            Error::UnauthorizedAccount(ownable::OwnableUnauthorizedAccount {
                account
            }) if account == ALICE
        ));
    }
}
