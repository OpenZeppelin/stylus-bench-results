#![cfg_attr(not(any(test, feature = "export-abi")), no_main)]
#![allow(clippy::result_large_err, clippy::needless_pass_by_value)]

extern crate alloc;

use alloc::vec::Vec;

use openzeppelin_stylus::{
    access::ownable::{self, IOwnable, Ownable},
    token::erc1155::{
        self,
        extensions::{Erc1155Supply, IErc1155Burnable, IErc1155Supply},
        IErc1155,
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
struct Erc1155Example {
    erc1155_supply: Erc1155Supply,
    ownable: Ownable,
    owner_initialized: StorageBool,
}

#[public]
#[implements(IErc1155<Error = erc1155::Error>, IErc1155Burnable<Error = erc1155::Error>, IErc1155Supply, IErc165, IOwnable)]
impl Erc1155Example {
    #[constructor]
    fn constructor(&mut self) -> Result<(), Vec<u8>> {
        self.ownable.constructor(self.vm().msg_sender())?;
        self.owner_initialized.set(true);
        Ok(())
    }

    /// Initializes ownership when upgrading a proxy created by the previous
    /// version. Call this atomically with the implementation upgrade.
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

    // Add token minting feature.
    fn mint(
        &mut self,
        to: Address,
        id: U256,
        value: U256,
        data: Bytes,
    ) -> Result<(), Vec<u8>> {
        self.ownable.only_owner()?;
        self.erc1155_supply._mint(to, id, value, &data).map_err(Into::into)
    }

    fn mint_batch(
        &mut self,
        to: Address,
        ids: Vec<U256>,
        values: Vec<U256>,
        data: Bytes,
    ) -> Result<(), Vec<u8>> {
        self.ownable.only_owner()?;
        self.erc1155_supply
            ._mint_batch(to, ids, values, &data)
            .map_err(Into::into)
    }
}

#[public]
impl IOwnable for Erc1155Example {
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
impl IErc1155 for Erc1155Example {
    type Error = erc1155::Error;

    fn balance_of(&self, account: Address, id: U256) -> U256 {
        self.erc1155_supply.balance_of(account, id)
    }

    fn balance_of_batch(
        &self,
        accounts: Vec<Address>,
        ids: Vec<U256>,
    ) -> Result<Vec<U256>, erc1155::Error> {
        self.erc1155_supply.balance_of_batch(accounts, ids)
    }

    fn set_approval_for_all(
        &mut self,
        operator: Address,
        approved: bool,
    ) -> Result<(), erc1155::Error> {
        self.erc1155_supply.set_approval_for_all(operator, approved)
    }

    fn is_approved_for_all(&self, account: Address, operator: Address) -> bool {
        self.erc1155_supply.is_approved_for_all(account, operator)
    }

    fn safe_transfer_from(
        &mut self,
        from: Address,
        to: Address,
        id: U256,
        value: U256,
        data: Bytes,
    ) -> Result<(), erc1155::Error> {
        self.erc1155_supply.safe_transfer_from(from, to, id, value, data)
    }

    fn safe_batch_transfer_from(
        &mut self,
        from: Address,
        to: Address,
        ids: Vec<U256>,
        values: Vec<U256>,
        data: Bytes,
    ) -> Result<(), erc1155::Error> {
        self.erc1155_supply
            .safe_batch_transfer_from(from, to, ids, values, data)
    }
}

#[public]
impl IErc1155Burnable for Erc1155Example {
    type Error = erc1155::Error;

    // Add token burning feature.
    fn burn(
        &mut self,
        account: Address,
        token_id: U256,
        value: U256,
    ) -> Result<(), erc1155::Error> {
        self.erc1155_supply.burn(account, token_id, value)
    }

    fn burn_batch(
        &mut self,
        account: Address,
        token_ids: Vec<U256>,
        values: Vec<U256>,
    ) -> Result<(), erc1155::Error> {
        self.erc1155_supply.burn_batch(account, token_ids, values)
    }
}

#[public]
impl IErc1155Supply for Erc1155Example {
    fn total_supply(&self, id: U256) -> U256 {
        self.erc1155_supply.total_supply(id)
    }

    #[selector(name = "totalSupply")]
    fn total_supply_all(&self) -> U256 {
        self.erc1155_supply.total_supply_all()
    }

    fn exists(&self, id: U256) -> bool {
        self.erc1155_supply.exists(id)
    }
}

#[public]
impl IErc165 for Erc1155Example {
    fn supports_interface(&self, interface_id: B32) -> bool {
        self.erc1155_supply.supports_interface(interface_id)
            || <Self as IOwnable>::interface_id() == interface_id
    }
}
