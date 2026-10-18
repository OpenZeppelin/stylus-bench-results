#![cfg_attr(not(any(test, feature = "export-abi")), no_main)]

extern crate alloc;

use alloc::vec::Vec;

use openzeppelin_stylus::access::ownable::{self, IOwnable, Ownable};
use openzeppelin_stylus::proxy::{
    beacon::IBeacon, erc1967::utils::Erc1967Utils,
};
use stylus_sdk::{
    abi::Bytes,
    alloy_primitives::Address,
    prelude::*,
    storage::{StorageAddress, StorageBool},
};

/// Test fixture that exposes ERC-1967 beacon upgrades and implements `IBeacon`.
#[entrypoint]
#[storage]
struct Erc1967BeaconRollbackExample {
    implementation: StorageAddress,
    ownable: Ownable,
    owner_initialized: StorageBool,
}

#[public]
#[implements(IBeacon, IOwnable)]
impl Erc1967BeaconRollbackExample {
    /// Store the implementation address returned by this fixture as a beacon.
    #[constructor]
    pub fn constructor(
        &mut self,
        implementation: Address,
    ) -> Result<(), Vec<u8>> {
        self.implementation.set(implementation);
        self.ownable.constructor(self.vm().msg_sender())?;
        self.owner_initialized.set(true);
        Ok(())
    }

    /// Initializes ownership on an existing deployment. Call this atomically
    /// with the implementation upgrade.
    fn initialize_owner(
        &mut self,
        initial_owner: Address,
    ) -> Result<(), Vec<u8>> {
        if self.owner_initialized.get() {
            return Err(ownable::Error::UnauthorizedAccount(
                ownable::OwnableUnauthorizedAccount {
                    account: self.vm().msg_sender(),
                },
            )
            .into());
        }
        self.ownable.constructor(initial_owner)?;
        self.owner_initialized.set(true);
        Ok(())
    }

    /// Return the beacon currently stored in the ERC-1967 slot.
    fn get_beacon(&self) -> Address {
        Erc1967Utils::get_beacon(self)
    }

    /// Upgrade this fixture's beacon slot and optionally delegate setup data.
    fn upgrade_beacon_to_and_call(
        &mut self,
        new_beacon: Address,
        data: Bytes,
    ) -> Result<(), Vec<u8>> {
        self.ownable.only_owner()?;
        Erc1967Utils::upgrade_beacon_to_and_call(self, new_beacon, &data)
            .map_err(Into::into)
    }
}

#[public]
impl IOwnable for Erc1967BeaconRollbackExample {
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

#[public]
impl IBeacon for Erc1967BeaconRollbackExample {
    fn implementation(&self) -> Result<Address, Vec<u8>> {
        Ok(self.implementation.get())
    }
}
