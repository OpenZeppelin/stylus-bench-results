#![cfg_attr(not(any(test, feature = "export-abi")), no_main)]
extern crate alloc;

use alloc::vec::Vec;

use openzeppelin_stylus::access::ownable::{self, IOwnable, Ownable};
use openzeppelin_stylus::{
    token::{
        erc1155,
        erc1155::{
            extensions::{
                Erc1155MetadataUri, Erc1155UriStorage, IErc1155MetadataUri,
            },
            Erc1155, IErc1155,
        },
    },
    utils::introspection::erc165::IErc165,
};
use stylus_sdk::{
    abi::Bytes,
    alloy_primitives::{aliases::B32, Address, U256},
    prelude::*,
    storage::StorageBool,
};

#[entrypoint]
#[storage]
struct Erc1155MetadataUriExample {
    erc1155: Erc1155,
    metadata_uri: Erc1155MetadataUri,
    uri_storage: Erc1155UriStorage,
    ownable: Ownable,
    owner_initialized: StorageBool,
}

#[public]
#[implements(IErc1155<Error = erc1155::Error>, IErc1155MetadataUri, IErc165, IOwnable)]
impl Erc1155MetadataUriExample {
    #[constructor]
    fn constructor(&mut self, uri: String) -> Result<(), Vec<u8>> {
        self.ownable.constructor(self.vm().msg_sender())?;
        self.owner_initialized.set(true);
        self.metadata_uri.constructor(uri);
        Ok(())
    }

    /// Initializes ownership when upgrading a proxy created by the v0.3.0
    /// example. Call this atomically with the implementation upgrade.
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

    #[selector(name = "setTokenURI")]
    fn set_token_uri(
        &mut self,
        token_id: U256,
        token_uri: String,
    ) -> Result<(), Vec<u8>> {
        self.ownable.only_owner()?;
        self.uri_storage.set_token_uri(token_id, token_uri, &self.metadata_uri);
        Ok(())
    }

    #[selector(name = "setBaseURI")]
    fn set_base_uri(&mut self, base_uri: String) -> Result<(), Vec<u8>> {
        self.ownable.only_owner()?;
        self.uri_storage.set_base_uri(base_uri);
        Ok(())
    }
}

#[public]
impl IOwnable for Erc1155MetadataUriExample {
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
impl IErc1155 for Erc1155MetadataUriExample {
    type Error = erc1155::Error;

    fn balance_of(&self, account: Address, id: U256) -> U256 {
        self.erc1155.balance_of(account, id)
    }

    fn balance_of_batch(
        &self,
        accounts: Vec<Address>,
        ids: Vec<U256>,
    ) -> Result<Vec<U256>, erc1155::Error> {
        self.erc1155.balance_of_batch(accounts, ids)
    }

    fn set_approval_for_all(
        &mut self,
        operator: Address,
        approved: bool,
    ) -> Result<(), erc1155::Error> {
        self.erc1155.set_approval_for_all(operator, approved)
    }

    fn is_approved_for_all(&self, account: Address, operator: Address) -> bool {
        self.erc1155.is_approved_for_all(account, operator)
    }

    fn safe_transfer_from(
        &mut self,
        from: Address,
        to: Address,
        id: U256,
        value: U256,
        data: Bytes,
    ) -> Result<(), erc1155::Error> {
        self.erc1155.safe_transfer_from(from, to, id, value, data)
    }

    fn safe_batch_transfer_from(
        &mut self,
        from: Address,
        to: Address,
        ids: Vec<U256>,
        values: Vec<U256>,
        data: Bytes,
    ) -> Result<(), erc1155::Error> {
        self.erc1155.safe_batch_transfer_from(from, to, ids, values, data)
    }
}

#[public]
impl IErc1155MetadataUri for Erc1155MetadataUriExample {
    fn uri(&self, token_id: U256) -> String {
        self.uri_storage.uri(token_id, &self.metadata_uri)
    }
}

#[public]
impl IErc165 for Erc1155MetadataUriExample {
    fn supports_interface(&self, interface_id: B32) -> bool {
        self.erc1155.supports_interface(interface_id)
            || self.metadata_uri.supports_interface(interface_id)
    }
}
