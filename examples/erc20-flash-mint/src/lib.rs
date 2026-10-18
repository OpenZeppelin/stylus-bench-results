#![cfg_attr(not(any(test, feature = "export-abi")), no_main)]
extern crate alloc;

use alloc::vec::Vec;

use openzeppelin_stylus::access::ownable::{self, IOwnable, Ownable};
use openzeppelin_stylus::{
    token::erc20::{
        extensions::{flash_mint, Erc20FlashMint, IErc3156FlashLender},
        Erc20, IErc20,
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
struct Erc20FlashMintExample {
    erc20: Erc20,
    flash_mint: Erc20FlashMint,
    ownable: Ownable,
    owner_initialized: StorageBool,
}

#[public]
#[implements(IErc20<Error = flash_mint::Error>, IErc3156FlashLender<Error = flash_mint::Error>, IErc165, IOwnable)]
impl Erc20FlashMintExample {
    #[constructor]
    fn constructor(&mut self) -> Result<(), Vec<u8>> {
        self.ownable.constructor(self.vm().msg_sender())?;
        self.owner_initialized.set(true);
        Ok(())
    }

    /// Initializes ownership when upgrading a proxy that predates this
    /// example's owner-controlled fee settings. Call this atomically with the
    /// implementation upgrade.
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

    fn mint(
        &mut self,
        to: Address,
        value: U256,
    ) -> Result<(), flash_mint::Error> {
        Ok(self.erc20._mint(to, value)?)
    }

    /// Sets the account that receives flash-loan fees. The owner controls this
    /// setting because it determines where value from flash loans is sent.
    fn set_flash_fee_receiver(
        &mut self,
        new_receiver: Address,
    ) -> Result<(), Vec<u8>> {
        self.ownable.only_owner()?;
        self.flash_mint.flash_fee_receiver_address.set(new_receiver);
        Ok(())
    }

    /// Sets the fixed fee charged for each flash loan. The owner controls this
    /// setting because it determines the cost paid by borrowers.
    fn set_flash_fee_value(&mut self, new_value: U256) -> Result<(), Vec<u8>> {
        self.ownable.only_owner()?;
        self.flash_mint.flash_fee_value.set(new_value);
        Ok(())
    }
}

#[public]
impl IOwnable for Erc20FlashMintExample {
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
impl IErc3156FlashLender for Erc20FlashMintExample {
    type Error = flash_mint::Error;

    fn max_flash_loan(&self, token: Address) -> U256 {
        self.flash_mint.max_flash_loan(token, &self.erc20)
    }

    fn flash_fee(
        &self,
        token: Address,
        value: U256,
    ) -> Result<U256, flash_mint::Error> {
        self.flash_mint.flash_fee(token, value)
    }

    fn flash_loan(
        &mut self,
        receiver: Address,
        token: Address,
        value: U256,
        data: Bytes,
    ) -> Result<bool, flash_mint::Error> {
        self.flash_mint.flash_loan(
            receiver,
            token,
            value,
            &data,
            &mut self.erc20,
        )
    }
}

#[public]
impl IErc20 for Erc20FlashMintExample {
    type Error = flash_mint::Error;

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
    ) -> Result<bool, flash_mint::Error> {
        Ok(self.erc20.transfer(to, value)?)
    }

    fn allowance(&self, owner: Address, spender: Address) -> U256 {
        self.erc20.allowance(owner, spender)
    }

    fn approve(
        &mut self,
        spender: Address,
        value: U256,
    ) -> Result<bool, flash_mint::Error> {
        Ok(self.erc20.approve(spender, value)?)
    }

    fn transfer_from(
        &mut self,
        from: Address,
        to: Address,
        value: U256,
    ) -> Result<bool, flash_mint::Error> {
        Ok(self.erc20.transfer_from(from, to, value)?)
    }
}

#[public]
impl IErc165 for Erc20FlashMintExample {
    fn supports_interface(&self, interface_id: B32) -> bool {
        <Self as IErc3156FlashLender>::interface_id() == interface_id
            || <Self as IOwnable>::interface_id() == interface_id
            || self.erc20.supports_interface(interface_id)
    }
}
