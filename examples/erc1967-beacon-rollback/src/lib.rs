#![cfg_attr(not(any(test, feature = "export-abi")), no_main)]

extern crate alloc;

use alloc::vec::Vec;

use openzeppelin_stylus::proxy::{
    beacon::IBeacon,
    erc1967::utils::{self, Erc1967Utils},
};
use stylus_sdk::{
    abi::Bytes, alloy_primitives::Address, prelude::*, storage::StorageAddress,
};

/// Test fixture that exposes ERC-1967 beacon upgrades and implements `IBeacon`.
#[entrypoint]
#[storage]
struct Erc1967BeaconRollbackExample {
    implementation: StorageAddress,
}

#[public]
#[implements(IBeacon)]
impl Erc1967BeaconRollbackExample {
    /// Store the implementation address returned by this fixture as a beacon.
    #[constructor]
    pub fn constructor(&mut self, implementation: Address) {
        self.implementation.set(implementation);
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
    ) -> Result<(), utils::Error> {
        Erc1967Utils::upgrade_beacon_to_and_call(self, new_beacon, &data)
    }
}

#[public]
impl IBeacon for Erc1967BeaconRollbackExample {
    fn implementation(&self) -> Result<Address, Vec<u8>> {
        Ok(self.implementation.get())
    }
}
