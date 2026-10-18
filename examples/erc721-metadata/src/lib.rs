#![cfg_attr(not(any(test, feature = "export-abi")), no_main)]
extern crate alloc;

use alloc::{string::String, vec::Vec};

use openzeppelin_stylus::access::ownable::{self, IOwnable, Ownable};
use openzeppelin_stylus::{
    token::erc721::{
        self,
        extensions::{
            Erc721Metadata, Erc721UriStorage, IErc721Burnable, IErc721Metadata,
            IErc721UriStorage,
        },
        Erc721, IErc721,
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
struct Erc721MetadataExample {
    erc721: Erc721,
    metadata: Erc721Metadata,
    uri_storage: Erc721UriStorage,
    ownable: Ownable,
    owner_initialized: StorageBool,
}

#[public]
#[implements(IErc721<Error = erc721::Error>, IErc721Burnable<Error = erc721::Error>, IErc721Metadata<Error = erc721::Error>, IErc165, IOwnable)]
impl Erc721MetadataExample {
    #[constructor]
    fn constructor(
        &mut self,
        name: String,
        symbol: String,
        base_uri: String,
        initial_owner: Address,
    ) -> Result<(), Vec<u8>> {
        self.ownable.constructor(initial_owner)?;
        self.owner_initialized.set(true);
        self.metadata.constructor(name, symbol);
        self.metadata.base_uri.set_str(base_uri);
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

    fn mint(&mut self, to: Address, token_id: U256) -> Result<(), Vec<u8>> {
        self.ownable.only_owner()?;
        self.erc721._mint(to, token_id).map_err(Into::into)
    }

    #[selector(name = "setTokenURI")]
    fn set_token_uri(
        &mut self,
        token_id: U256,
        token_uri: String,
    ) -> Result<(), Vec<u8>> {
        self.ownable.only_owner()?;
        self.uri_storage._set_token_uri(token_id, token_uri);
        Ok(())
    }
}

#[public]
impl IOwnable for Erc721MetadataExample {
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
impl IErc721 for Erc721MetadataExample {
    type Error = erc721::Error;

    fn balance_of(&self, owner: Address) -> Result<U256, erc721::Error> {
        self.erc721.balance_of(owner)
    }

    fn owner_of(&self, token_id: U256) -> Result<Address, erc721::Error> {
        self.erc721.owner_of(token_id)
    }

    fn safe_transfer_from(
        &mut self,
        from: Address,
        to: Address,
        token_id: U256,
    ) -> Result<(), erc721::Error> {
        self.erc721.safe_transfer_from(from, to, token_id)
    }

    fn safe_transfer_from_with_data(
        &mut self,
        from: Address,
        to: Address,
        token_id: U256,
        data: Bytes,
    ) -> Result<(), erc721::Error> {
        self.erc721.safe_transfer_from_with_data(from, to, token_id, data)
    }

    fn transfer_from(
        &mut self,
        from: Address,
        to: Address,
        token_id: U256,
    ) -> Result<(), erc721::Error> {
        self.erc721.transfer_from(from, to, token_id)
    }

    fn approve(
        &mut self,
        to: Address,
        token_id: U256,
    ) -> Result<(), erc721::Error> {
        self.erc721.approve(to, token_id)
    }

    fn set_approval_for_all(
        &mut self,
        to: Address,
        approved: bool,
    ) -> Result<(), erc721::Error> {
        self.erc721.set_approval_for_all(to, approved)
    }

    fn get_approved(&self, token_id: U256) -> Result<Address, erc721::Error> {
        self.erc721.get_approved(token_id)
    }

    fn is_approved_for_all(&self, owner: Address, operator: Address) -> bool {
        self.erc721.is_approved_for_all(owner, operator)
    }
}

#[public]
impl IErc721Burnable for Erc721MetadataExample {
    type Error = erc721::Error;

    fn burn(&mut self, token_id: U256) -> Result<(), erc721::Error> {
        self.erc721.burn(token_id)
    }
}

#[public]
impl IErc721Metadata for Erc721MetadataExample {
    type Error = erc721::Error;

    fn name(&self) -> String {
        self.metadata.name()
    }

    fn symbol(&self) -> String {
        self.metadata.symbol()
    }

    #[selector(name = "tokenURI")]
    fn token_uri(&self, token_id: U256) -> Result<String, erc721::Error> {
        self.uri_storage.token_uri(token_id, &self.erc721, &self.metadata)
    }
}

impl IErc721UriStorage for Erc721MetadataExample {}

#[public]
impl IErc165 for Erc721MetadataExample {
    fn supports_interface(&self, interface_id: B32) -> bool {
        self.erc721.supports_interface(interface_id)
            || <Self as IErc721Metadata>::interface_id() == interface_id
            || <Self as IOwnable>::interface_id() == interface_id
    }
}
