#![cfg_attr(not(any(test, feature = "export-abi")), no_main)]
#![allow(clippy::needless_pass_by_value)]

extern crate alloc;

use alloc::vec::Vec;
use alloy_primitives::{
    aliases::{B32, U96},
    Address, U256,
};
use openzeppelin_stylus::{
    access::ownable::{self, IOwnable, Ownable},
    token::erc721::{
        extensions::{consecutive, Erc721Consecutive, IErc721Burnable},
        IErc721,
    },
    utils::introspection::erc165::IErc165,
};
use stylus_sdk::prelude::errors::MethodError;
use stylus_sdk::{abi::Bytes, prelude::*, storage::StorageBool};

#[entrypoint]
#[storage]
struct Erc721ConsecutiveExample {
    erc721: Erc721Consecutive,
    ownable: Ownable,
    owner_initialized: StorageBool,
}

#[public]
#[implements(IErc721<Error = consecutive::Error>, IErc721Burnable<Error = consecutive::Error>, IErc165, IOwnable)]
impl Erc721ConsecutiveExample {
    #[constructor]
    fn constructor(
        &mut self,
        receivers: Vec<Address>,
        amounts: Vec<U96>,
        first_consecutive_id: U96,
        max_batch_size: U96,
    ) -> Result<(), Vec<u8>> {
        self.ownable.constructor(self.vm().msg_sender())?;
        self.owner_initialized.set(true);
        self.erc721.first_consecutive_id.set(first_consecutive_id);
        self.erc721.max_batch_size.set(max_batch_size);
        self.erc721
            ._mint_consecutive_batches(
                receivers.iter().copied().zip(amounts.iter().copied()),
            )
            .map_err(|error| error.encode())?;
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
}

#[public]
impl IOwnable for Erc721ConsecutiveExample {
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
impl IErc721 for Erc721ConsecutiveExample {
    type Error = consecutive::Error;

    fn balance_of(&self, owner: Address) -> Result<U256, consecutive::Error> {
        self.erc721.balance_of(owner)
    }

    fn owner_of(&self, token_id: U256) -> Result<Address, consecutive::Error> {
        self.erc721.owner_of(token_id)
    }

    fn safe_transfer_from(
        &mut self,
        from: Address,
        to: Address,
        token_id: U256,
    ) -> Result<(), consecutive::Error> {
        self.erc721.safe_transfer_from(from, to, token_id)
    }

    fn safe_transfer_from_with_data(
        &mut self,
        from: Address,
        to: Address,
        token_id: U256,
        data: Bytes,
    ) -> Result<(), consecutive::Error> {
        self.erc721.safe_transfer_from_with_data(from, to, token_id, data)
    }

    fn transfer_from(
        &mut self,
        from: Address,
        to: Address,
        token_id: U256,
    ) -> Result<(), consecutive::Error> {
        self.erc721.transfer_from(from, to, token_id)
    }

    fn approve(
        &mut self,
        to: Address,
        token_id: U256,
    ) -> Result<(), consecutive::Error> {
        self.erc721.approve(to, token_id)
    }

    fn set_approval_for_all(
        &mut self,
        to: Address,
        approved: bool,
    ) -> Result<(), consecutive::Error> {
        self.erc721.set_approval_for_all(to, approved)
    }

    fn get_approved(
        &self,
        token_id: U256,
    ) -> Result<Address, consecutive::Error> {
        self.erc721.get_approved(token_id)
    }

    fn is_approved_for_all(&self, owner: Address, operator: Address) -> bool {
        self.erc721.is_approved_for_all(owner, operator)
    }
}

#[public]
impl IErc721Burnable for Erc721ConsecutiveExample {
    type Error = consecutive::Error;

    fn burn(&mut self, token_id: U256) -> Result<(), consecutive::Error> {
        self.erc721._update(Address::ZERO, token_id, self.vm().msg_sender())?;
        Ok(())
    }
}

#[public]
impl IErc165 for Erc721ConsecutiveExample {
    fn supports_interface(&self, interface_id: B32) -> bool {
        self.erc721.supports_interface(interface_id)
    }
}
