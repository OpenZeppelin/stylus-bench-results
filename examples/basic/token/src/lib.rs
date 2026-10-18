#![cfg_attr(not(any(test, feature = "export-abi")), no_main)]
extern crate alloc;

use alloc::vec::Vec;

use openzeppelin_stylus::{
    access::ownable::{self, IOwnable, Ownable},
    token::erc20::{
        self,
        extensions::{Erc20Metadata, IErc20Metadata},
        Erc20, IErc20,
    },
    utils::introspection::erc165::IErc165,
};
use stylus_sdk::{
    alloy_primitives::{aliases::B32, Address, U256, U8},
    prelude::*,
    storage::StorageBool,
};

#[entrypoint]
#[storage]
struct Erc20Example {
    erc20: Erc20,
    metadata: Erc20Metadata,
    ownable: Ownable,
    owner_initialized: StorageBool,
}

#[public]
#[implements(IErc20<Error = erc20::Error>, IErc20Metadata, IErc165, IOwnable)]
impl Erc20Example {
    #[constructor]
    fn constructor(
        &mut self,
        name: String,
        symbol: String,
    ) -> Result<(), Vec<u8>> {
        self.ownable.constructor(self.vm().msg_sender())?;
        self.owner_initialized.set(true);
        self.metadata.constructor(name, symbol);
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

    fn mint(&mut self, account: Address, value: U256) -> Result<(), Vec<u8>> {
        self.ownable.only_owner()?;
        self.erc20._mint(account, value).map_err(Into::into)
    }
}

#[public]
impl IOwnable for Erc20Example {
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
impl IErc20 for Erc20Example {
    type Error = erc20::Error;

    fn total_supply(&self) -> U256 {
        self.erc20.total_supply()
    }

    fn balance_of(&self, account: Address) -> U256 {
        self.erc20.balance_of(account)
    }

    fn transfer(
        &mut self,
        to: Address,
        value: U256,
    ) -> Result<bool, erc20::Error> {
        self.erc20.transfer(to, value)
    }

    fn allowance(&self, owner: Address, spender: Address) -> U256 {
        self.erc20.allowance(owner, spender)
    }

    fn approve(
        &mut self,
        spender: Address,
        value: U256,
    ) -> Result<bool, erc20::Error> {
        self.erc20.approve(spender, value)
    }

    fn transfer_from(
        &mut self,
        from: Address,
        to: Address,
        value: U256,
    ) -> Result<bool, erc20::Error> {
        self.erc20.transfer_from(from, to, value)
    }
}

#[public]
impl IErc20Metadata for Erc20Example {
    fn name(&self) -> String {
        self.metadata.name()
    }

    fn symbol(&self) -> String {
        self.metadata.symbol()
    }

    fn decimals(&self) -> U8 {
        self.metadata.decimals()
    }
}

#[public]
impl IErc165 for Erc20Example {
    fn supports_interface(&self, interface_id: B32) -> bool {
        self.erc20.supports_interface(interface_id)
            || self.metadata.supports_interface(interface_id)
            || <Self as IOwnable>::interface_id() == interface_id
    }
}
