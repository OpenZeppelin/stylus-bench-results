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
/// [`BeaconProxy`][BeaconProxy] to determine their implementation /// which is where they will delegate all function calls.
///
/// An owner is able to change the implementation the beacon points to, thus
/// upgrading the proxies that use this beacon.
///
/// [BeaconProxy]: super::BeaconProxy
#[public]
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
        if initial_owner == Address::ZERO {
            return Err(Error::InvalidOwner(ownable::OwnableInvalidOwner {
                owner: initial_owner,
            }));
        }
        if self.vm().code_size(implementation) == 0 {
            return Err(Error::InvalidImplementation(
                BeaconInvalidImplementation { implementation },
            ));
        }

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
        if self.vm().code_size(new_implementation) == 0 {
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

#[cfg(test)]
mod tests {
    use stylus_sdk::alloy_primitives::Address;
    use stylus_sdk::testing::TestVM;

    use super::*;
    use crate::proxy::beacon::IBeacon;

    use crate::test_utils::assert_emitted;

    #[test]
    fn constructor() {
        let beacon_vm = TestVM::default();
        let beacon_address = Address::from([160u8; 20]);
        beacon_vm.set_contract_address(beacon_address);
        let mut beacon = UpgradeableBeacon::from(&beacon_vm);
        let erc20_address = Address::from([161u8; 20]);
        beacon_vm.set_code(erc20_address, vec![1]);
        let alice = Address::from([1u8; 20]);

        ({
            beacon_vm.set_sender(alice);
            &mut beacon
        })
        .constructor(erc20_address, alice)
        .unwrap();

        let implementation = ({
            beacon_vm.set_sender(alice);
            &mut beacon
        })
        .implementation()
        .expect("should be able to get implementation");
        assert_eq!(implementation, erc20_address);

        let owner = ({
            beacon_vm.set_sender(alice);
            &mut beacon
        })
        .owner();
        assert_eq!(owner, alice);
    }

    #[test]
    fn constructor_with_invalid_implementation() {
        let beacon_vm = TestVM::default();
        let beacon_address = Address::from([160u8; 20]);
        beacon_vm.set_contract_address(beacon_address);
        let mut beacon = UpgradeableBeacon::from(&beacon_vm);
        let alice = Address::from([1u8; 20]);

        let invalid_address = alice;
        let err = ({
            beacon_vm.set_sender(alice);
            &mut beacon
        })
        .constructor(invalid_address, alice)
        .expect_err("should fail when constructor has invalid implementation");

        assert!(matches!(
            err,
            Error::InvalidImplementation(BeaconInvalidImplementation {
                implementation,
            }) if implementation == invalid_address
        ));

        assert_eq!(beacon.owner(), Address::ZERO);
        assert!(beacon_vm.get_emitted_logs().is_empty());
    }

    #[test]
    fn constructor_with_zero_owner() {
        let beacon_vm = TestVM::default();
        let beacon_address = Address::from([160u8; 20]);
        beacon_vm.set_contract_address(beacon_address);
        let mut beacon = UpgradeableBeacon::from(&beacon_vm);
        let erc20_address = Address::from([161u8; 20]);
        beacon_vm.set_code(erc20_address, vec![1]);
        let alice = Address::from([1u8; 20]);

        let err = ({
            beacon_vm.set_sender(alice);
            &mut beacon
        })
        .constructor(erc20_address, Address::ZERO)
        .expect_err("should fail when constructor has zero owner");

        assert!(matches!(
            err,
            Error::InvalidOwner(ownable::OwnableInvalidOwner {
                owner,
            }) if owner == Address::ZERO
        ));
    }

    #[test]
    fn constructor_with_zero_implementation() {
        let beacon_vm = TestVM::default();
        let beacon_address = Address::from([160u8; 20]);
        beacon_vm.set_contract_address(beacon_address);
        let mut beacon = UpgradeableBeacon::from(&beacon_vm);
        let alice = Address::from([1u8; 20]);

        let err = ({
            beacon_vm.set_sender(alice);
            &mut beacon
        })
        .constructor(Address::ZERO, alice)
        .expect_err("should fail when constructor has zero implementation");

        assert!(matches!(
            err,
            Error::InvalidImplementation(BeaconInvalidImplementation {
                implementation,
            }) if implementation == Address::ZERO
        ));
    }

    #[test]
    fn upgrade_to_valid_implementation() {
        let beacon_vm = TestVM::default();
        let beacon_address = Address::from([160u8; 20]);
        beacon_vm.set_contract_address(beacon_address);
        let mut beacon = UpgradeableBeacon::from(&beacon_vm);
        let erc20_address = Address::from([161u8; 20]);
        beacon_vm.set_code(erc20_address, vec![1]);
        let erc20_2_address = Address::from([162u8; 20]);
        beacon_vm.set_code(erc20_2_address, vec![1]);
        let alice = Address::from([1u8; 20]);

        ({
            beacon_vm.set_sender(alice);
            &mut beacon
        })
        .constructor(erc20_address, alice)
        .unwrap();

        // upgrade to new implementation.
        ({
            beacon_vm.set_sender(alice);
            &mut beacon
        })
        .upgrade_to(erc20_2_address)
        .expect("should be able to upgrade to valid implementation");

        let implementation = ({
            beacon_vm.set_sender(alice);
            &mut beacon
        })
        .implementation()
        .expect("should be able to get implementation");
        assert_eq!(implementation, erc20_2_address);

        // verify event was emitted.
        assert_emitted(
            &beacon_vm,
            &Upgraded { implementation: erc20_2_address },
        );
    }

    #[test]
    fn upgrade_to_invalid_implementation() {
        let beacon_vm = TestVM::default();
        let beacon_address = Address::from([160u8; 20]);
        beacon_vm.set_contract_address(beacon_address);
        let mut beacon = UpgradeableBeacon::from(&beacon_vm);
        let erc20_address = Address::from([161u8; 20]);
        beacon_vm.set_code(erc20_address, vec![1]);
        let alice = Address::from([1u8; 20]);

        ({
            beacon_vm.set_sender(alice);
            &mut beacon
        })
        .constructor(erc20_address, alice)
        .unwrap();

        // try to upgrade to address with no code.
        let invalid_address = alice;
        let err = ({
            beacon_vm.set_sender(alice);
            &mut beacon
        })
        .upgrade_to(invalid_address)
        .expect_err("should fail when upgrading to invalid implementation");

        assert!(matches!(
            err,
            Error::InvalidImplementation(BeaconInvalidImplementation {
                implementation
            }) if implementation == invalid_address
        ));

        // implementation should remain unchanged.
        let implementation = ({
            beacon_vm.set_sender(alice);
            &mut beacon
        })
        .implementation()
        .expect("should be able to get implementation");
        assert_eq!(implementation, erc20_address);
    }

    #[test]
    fn upgrade_to_unauthorized() {
        let beacon_vm = TestVM::default();
        let beacon_address = Address::from([160u8; 20]);
        beacon_vm.set_contract_address(beacon_address);
        let mut beacon = UpgradeableBeacon::from(&beacon_vm);
        let erc20_address = Address::from([161u8; 20]);
        beacon_vm.set_code(erc20_address, vec![1]);
        let erc20_2_address = Address::from([162u8; 20]);
        beacon_vm.set_code(erc20_2_address, vec![1]);
        let alice = Address::from([1u8; 20]);
        let bob = Address::from([2u8; 20]);

        ({
            beacon_vm.set_sender(alice);
            &mut beacon
        })
        .constructor(erc20_address, alice)
        .unwrap();

        // try to upgrade from non-owner account.
        let err = ({
            beacon_vm.set_sender(bob);
            &mut beacon
        })
        .upgrade_to(erc20_2_address)
        .expect_err("should fail when called by non-owner");

        assert!(matches!(
            err,
            Error::UnauthorizedAccount(ownable::OwnableUnauthorizedAccount {
                account,
            }) if account == bob
        ));

        // implementation should remain unchanged.
        let implementation = ({
            beacon_vm.set_sender(alice);
            &mut beacon
        })
        .implementation()
        .expect("should be able to get implementation");
        assert_eq!(implementation, erc20_address);
    }

    #[test]
    fn upgrade_to_same_implementation() {
        let beacon_vm = TestVM::default();
        let beacon_address = Address::from([160u8; 20]);
        beacon_vm.set_contract_address(beacon_address);
        let mut beacon = UpgradeableBeacon::from(&beacon_vm);
        let erc20_address = Address::from([161u8; 20]);
        beacon_vm.set_code(erc20_address, vec![1]);
        let alice = Address::from([1u8; 20]);

        ({
            beacon_vm.set_sender(alice);
            &mut beacon
        })
        .constructor(erc20_address, alice)
        .unwrap();

        // upgrade to the same implementation.
        ({
            beacon_vm.set_sender(alice);
            &mut beacon
        })
        .upgrade_to(erc20_address)
        .expect("should be able to upgrade to same implementation");

        // event should still be emitted.
        assert_emitted(&beacon_vm, &Upgraded { implementation: erc20_address });

        let implementation = ({
            beacon_vm.set_sender(alice);
            &mut beacon
        })
        .implementation()
        .expect("should be able to get implementation");
        assert_eq!(implementation, erc20_address);
    }

    #[test]
    fn upgrade_to_zero_address() {
        let beacon_vm = TestVM::default();
        let beacon_address = Address::from([160u8; 20]);
        beacon_vm.set_contract_address(beacon_address);
        let mut beacon = UpgradeableBeacon::from(&beacon_vm);
        let erc20_address = Address::from([161u8; 20]);
        beacon_vm.set_code(erc20_address, vec![1]);
        let alice = Address::from([1u8; 20]);

        ({
            beacon_vm.set_sender(alice);
            &mut beacon
        })
        .constructor(erc20_address, alice)
        .unwrap();

        // try to upgrade to [`Address::ZERO`].
        let err = ({
            beacon_vm.set_sender(alice);
            &mut beacon
        })
        .upgrade_to(Address::ZERO)
        .expect_err("should fail when upgrading to zero address");

        assert!(matches!(
            err,
            Error::InvalidImplementation(BeaconInvalidImplementation {
                implementation,
            }) if implementation == Address::ZERO
        ));

        // implementation should remain unchanged.
        let implementation = ({
            beacon_vm.set_sender(alice);
            &mut beacon
        })
        .implementation()
        .expect("should be able to get implementation");
        assert_eq!(implementation, erc20_address);
    }

    #[test]
    fn multiple_upgrades_emit_events() {
        let beacon_vm = TestVM::default();
        let beacon_address = Address::from([160u8; 20]);
        beacon_vm.set_contract_address(beacon_address);
        let mut beacon = UpgradeableBeacon::from(&beacon_vm);
        let erc20_address = Address::from([161u8; 20]);
        beacon_vm.set_code(erc20_address, vec![1]);
        let erc20_2_address = Address::from([162u8; 20]);
        beacon_vm.set_code(erc20_2_address, vec![1]);
        let erc20_3_address = Address::from([163u8; 20]);
        beacon_vm.set_code(erc20_3_address, vec![1]);
        let alice = Address::from([1u8; 20]);

        ({
            beacon_vm.set_sender(alice);
            &mut beacon
        })
        .constructor(erc20_address, alice)
        .unwrap();

        // first upgrade.
        ({
            beacon_vm.set_sender(alice);
            &mut beacon
        })
        .upgrade_to(erc20_2_address)
        .expect("should be able to upgrade first time");

        assert_emitted(
            &beacon_vm,
            &Upgraded { implementation: erc20_2_address },
        );

        // second upgrade.
        ({
            beacon_vm.set_sender(alice);
            &mut beacon
        })
        .upgrade_to(erc20_3_address)
        .expect("should be able to upgrade second time");

        assert_emitted(
            &beacon_vm,
            &Upgraded { implementation: erc20_3_address },
        );

        let implementation = ({
            beacon_vm.set_sender(alice);
            &mut beacon
        })
        .implementation()
        .expect("should be able to get implementation");
        assert_eq!(implementation, erc20_3_address);
    }

    #[test]
    fn transfer_ownership() {
        let beacon_vm = TestVM::default();
        let beacon_address = Address::from([160u8; 20]);
        beacon_vm.set_contract_address(beacon_address);
        let mut beacon = UpgradeableBeacon::from(&beacon_vm);
        let erc20_address = Address::from([161u8; 20]);
        beacon_vm.set_code(erc20_address, vec![1]);
        let erc20_2_address = Address::from([162u8; 20]);
        beacon_vm.set_code(erc20_2_address, vec![1]);
        let alice = Address::from([1u8; 20]);
        let bob = Address::from([2u8; 20]);

        ({
            beacon_vm.set_sender(alice);
            &mut beacon
        })
        .constructor(erc20_address, alice)
        .unwrap();

        // transfer ownership to bob.
        ({
            beacon_vm.set_sender(alice);
            &mut beacon
        })
        .transfer_ownership(bob)
        .expect("should be able to transfer ownership");

        let owner = ({
            beacon_vm.set_sender(alice);
            &mut beacon
        })
        .owner();
        assert_eq!(owner, bob);

        // bob should now be able to upgrade.
        ({
            beacon_vm.set_sender(bob);
            &mut beacon
        })
        .upgrade_to(erc20_2_address)
        .expect("new owner should be able to upgrade");

        // alice should not be able to upgrade.
        let err = ({
            beacon_vm.set_sender(alice);
            &mut beacon
        })
        .upgrade_to(erc20_address)
        .expect_err("should fail when called by non-owner");

        assert!(matches!(
            err,
            Error::UnauthorizedAccount(ownable::OwnableUnauthorizedAccount {
                account,
            }) if account == alice
        ));
    }

    #[test]
    fn transfer_ownership_to_zero_address() {
        let beacon_vm = TestVM::default();
        let beacon_address = Address::from([160u8; 20]);
        beacon_vm.set_contract_address(beacon_address);
        let mut beacon = UpgradeableBeacon::from(&beacon_vm);
        let erc20_address = Address::from([161u8; 20]);
        beacon_vm.set_code(erc20_address, vec![1]);
        let alice = Address::from([1u8; 20]);

        ({
            beacon_vm.set_sender(alice);
            &mut beacon
        })
        .constructor(erc20_address, alice)
        .unwrap();

        // try to transfer ownership to [`Address::ZERO`].
        let err = ({
            beacon_vm.set_sender(alice);
            &mut beacon
        })
        .transfer_ownership(Address::ZERO)
        .expect_err("should fail when transferring ownership to zero address");

        // the error should be from the ownable contract.
        assert_eq!(
            err,
            Error::InvalidOwner(ownable::OwnableInvalidOwner {
                owner: Address::ZERO,
            })
            .encode()
        );

        // ownership should remain unchanged.
        let owner = ({
            beacon_vm.set_sender(alice);
            &mut beacon
        })
        .owner();
        assert_eq!(owner, alice);
    }

    #[test]
    fn transfer_ownership_unauthorized() {
        let beacon_vm = TestVM::default();
        let beacon_address = Address::from([160u8; 20]);
        beacon_vm.set_contract_address(beacon_address);
        let mut beacon = UpgradeableBeacon::from(&beacon_vm);
        let erc20_address = Address::from([161u8; 20]);
        beacon_vm.set_code(erc20_address, vec![1]);
        let alice = Address::from([1u8; 20]);
        let bob = Address::from([2u8; 20]);
        let charlie = Address::from([3u8; 20]);

        ({
            beacon_vm.set_sender(alice);
            &mut beacon
        })
        .constructor(erc20_address, alice)
        .unwrap();

        // try to transfer ownership from non-owner account.
        let err = ({
            beacon_vm.set_sender(bob);
            &mut beacon
        })
        .transfer_ownership(charlie)
        .expect_err("should fail when called by non-owner");

        assert_eq!(
            err,
            Error::UnauthorizedAccount(ownable::OwnableUnauthorizedAccount {
                account: bob
            })
            .encode()
        );

        // ownership should remain unchanged.
        let owner = ({
            beacon_vm.set_sender(alice);
            &mut beacon
        })
        .owner();
        assert_eq!(owner, alice);
    }

    #[test]
    fn renounce_ownership() {
        let beacon_vm = TestVM::default();
        let beacon_address = Address::from([160u8; 20]);
        beacon_vm.set_contract_address(beacon_address);
        let mut beacon = UpgradeableBeacon::from(&beacon_vm);
        let erc20_address = Address::from([161u8; 20]);
        beacon_vm.set_code(erc20_address, vec![1]);
        let erc20_2_address = Address::from([162u8; 20]);
        beacon_vm.set_code(erc20_2_address, vec![1]);
        let alice = Address::from([1u8; 20]);

        ({
            beacon_vm.set_sender(alice);
            &mut beacon
        })
        .constructor(erc20_address, alice)
        .unwrap();

        // renounce ownership.
        ({
            beacon_vm.set_sender(alice);
            &mut beacon
        })
        .renounce_ownership()
        .expect("should be able to renounce ownership");

        let owner = ({
            beacon_vm.set_sender(alice);
            &mut beacon
        })
        .owner();
        assert_eq!(owner, Address::ZERO);

        // no one should be able to upgrade now.
        let err = ({
            beacon_vm.set_sender(alice);
            &mut beacon
        })
        .upgrade_to(erc20_2_address)
        .expect_err("should fail when no owner exists");

        assert!(matches!(
            err,
            Error::UnauthorizedAccount(ownable::OwnableUnauthorizedAccount {
                account,
            }) if account == alice
        ));
    }

    #[test]
    fn renounce_ownership_unauthorized() {
        let beacon_vm = TestVM::default();
        let beacon_address = Address::from([160u8; 20]);
        beacon_vm.set_contract_address(beacon_address);
        let mut beacon = UpgradeableBeacon::from(&beacon_vm);
        let erc20_address = Address::from([161u8; 20]);
        beacon_vm.set_code(erc20_address, vec![1]);
        let alice = Address::from([1u8; 20]);
        let bob = Address::from([2u8; 20]);

        ({
            beacon_vm.set_sender(alice);
            &mut beacon
        })
        .constructor(erc20_address, alice)
        .unwrap();

        // try to renounce ownership from non-owner account.
        let err = ({
            beacon_vm.set_sender(bob);
            &mut beacon
        })
        .renounce_ownership()
        .expect_err("should fail when called by non-owner");

        assert_eq!(
            err,
            Error::UnauthorizedAccount(ownable::OwnableUnauthorizedAccount {
                account: bob
            })
            .encode()
        );

        // ownership should remain unchanged.
        let owner = ({
            beacon_vm.set_sender(alice);
            &mut beacon
        })
        .owner();
        assert_eq!(owner, alice);
    }

    #[test]
    fn upgrade_after_ownership_transfer_chain() {
        let beacon_vm = TestVM::default();
        let beacon_address = Address::from([160u8; 20]);
        beacon_vm.set_contract_address(beacon_address);
        let mut beacon = UpgradeableBeacon::from(&beacon_vm);
        let erc20_address = Address::from([161u8; 20]);
        beacon_vm.set_code(erc20_address, vec![1]);
        let erc20_2_address = Address::from([162u8; 20]);
        beacon_vm.set_code(erc20_2_address, vec![1]);
        let erc20_3_address = Address::from([163u8; 20]);
        beacon_vm.set_code(erc20_3_address, vec![1]);
        let alice = Address::from([1u8; 20]);
        let bob = Address::from([2u8; 20]);
        let charlie = Address::from([3u8; 20]);

        ({
            beacon_vm.set_sender(alice);
            &mut beacon
        })
        .constructor(erc20_address, alice)
        .unwrap();

        // alice transfers ownership to bob.
        ({
            beacon_vm.set_sender(alice);
            &mut beacon
        })
        .transfer_ownership(bob)
        .expect("should be able to transfer ownership to bob");

        // bob transfers ownership to charlie.
        ({
            beacon_vm.set_sender(bob);
            &mut beacon
        })
        .transfer_ownership(charlie)
        .expect("should be able to transfer ownership to charlie");

        // charlie should be able to upgrade.
        ({
            beacon_vm.set_sender(charlie);
            &mut beacon
        })
        .upgrade_to(erc20_2_address)
        .expect("charlie should be able to upgrade");

        let implementation = ({
            beacon_vm.set_sender(alice);
            &mut beacon
        })
        .implementation()
        .expect("should be able to get implementation");
        assert_eq!(implementation, erc20_2_address);

        // alice and bob should not be able to upgrade anymore.
        let err = ({
            beacon_vm.set_sender(alice);
            &mut beacon
        })
        .upgrade_to(erc20_3_address)
        .expect_err("alice should not be able to upgrade");
        assert!(matches!(
            err,
            Error::UnauthorizedAccount(ownable::OwnableUnauthorizedAccount {
                account,
            }) if account == alice
        ));

        let err = ({
            beacon_vm.set_sender(bob);
            &mut beacon
        })
        .upgrade_to(erc20_3_address)
        .expect_err("bob should not be able to upgrade");
        assert!(matches!(
            err,
            Error::UnauthorizedAccount(ownable::OwnableUnauthorizedAccount {
                account,
            }) if account == bob
        ));
    }

    #[test]
    fn upgrade_after_renounce_and_transfer() {
        let beacon_vm = TestVM::default();
        let beacon_address = Address::from([160u8; 20]);
        beacon_vm.set_contract_address(beacon_address);
        let mut beacon = UpgradeableBeacon::from(&beacon_vm);
        let erc20_address = Address::from([161u8; 20]);
        beacon_vm.set_code(erc20_address, vec![1]);
        let erc20_2_address = Address::from([162u8; 20]);
        beacon_vm.set_code(erc20_2_address, vec![1]);
        let alice = Address::from([1u8; 20]);

        ({
            beacon_vm.set_sender(alice);
            &mut beacon
        })
        .constructor(erc20_address, alice)
        .unwrap();

        // alice renounces ownership.
        ({
            beacon_vm.set_sender(alice);
            &mut beacon
        })
        .renounce_ownership()
        .expect("should be able to renounce ownership");

        // no one should be able to upgrade.
        let err = ({
            beacon_vm.set_sender(alice);
            &mut beacon
        })
        .upgrade_to(erc20_2_address)
        .expect_err("should fail when no owner exists");
        assert!(matches!(
            err,
            Error::UnauthorizedAccount(ownable::OwnableUnauthorizedAccount {
                account,
            }) if account == alice
        ));
    }
}
