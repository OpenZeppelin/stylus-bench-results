#![cfg_attr(not(any(test, feature = "export-abi")), no_main)]
extern crate alloc;

// This integration-test example authorizes its explicitly supplied initial
// owner only. Production consumers should add their governance policy.

use alloy_sol_types::sol;
use openzeppelin_stylus::access::ownable::{self, Ownable};
use openzeppelin_stylus::token::erc20::utils::safe_erc20::{
    self, ISafeErc20, SafeErc20,
};
use stylus_sdk::{
    abi::Bytes,
    alloy_primitives::{Address, U256},
    prelude::*,
};

sol! {
    #[derive(Debug)]
    #[allow(missing_docs)]
    event True();
    #[derive(Debug)]
    #[allow(missing_docs)]
    event False();
}

#[derive(SolidityError, Debug)]
enum Error {
    UnauthorizedAccount(ownable::OwnableUnauthorizedAccount),
    InvalidOwner(ownable::OwnableInvalidOwner),
    SafeErc20FailedOperation(safe_erc20::SafeErc20FailedOperation),
    SafeErc20FailedDecreaseAllowance(
        safe_erc20::SafeErc20FailedDecreaseAllowance,
    ),
}

impl From<safe_erc20::Error> for Error {
    fn from(value: safe_erc20::Error) -> Self {
        match value {
            safe_erc20::Error::SafeErc20FailedOperation(error) => {
                Self::SafeErc20FailedOperation(error)
            }
            safe_erc20::Error::SafeErc20FailedDecreaseAllowance(error) => {
                Self::SafeErc20FailedDecreaseAllowance(error)
            }
        }
    }
}

impl From<ownable::Error> for Error {
    fn from(value: ownable::Error) -> Self {
        match value {
            ownable::Error::UnauthorizedAccount(error) => {
                Self::UnauthorizedAccount(error)
            }
            ownable::Error::InvalidOwner(error) => Self::InvalidOwner(error),
        }
    }
}

#[entrypoint]
#[storage]
struct SafeErc20Example {
    safe_erc20: SafeErc20,
    ownable: Ownable,
}

#[public]
#[implements(ISafeErc20<Error = Error>)]
impl SafeErc20Example {
    #[constructor]
    fn constructor(&mut self, initial_owner: Address) -> Result<(), Error> {
        self.ownable.constructor(initial_owner).map_err(Into::into)
    }
}

impl SafeErc20Example {
    fn only_owner(&self) -> Result<(), Error> {
        self.ownable.only_owner().map_err(Into::into)
    }
}

#[public]
impl ISafeErc20 for SafeErc20Example {
    type Error = Error;

    fn safe_transfer(
        &mut self,
        token: Address,
        to: Address,
        value: U256,
    ) -> Result<(), Error> {
        self.only_owner()?;
        self.safe_erc20.safe_transfer(token, to, value).map_err(Into::into)
    }

    fn safe_transfer_from(
        &mut self,
        token: Address,
        from: Address,
        to: Address,
        value: U256,
    ) -> Result<(), Error> {
        self.only_owner()?;
        self.safe_erc20
            .safe_transfer_from(token, from, to, value)
            .map_err(Into::into)
    }

    fn try_safe_transfer(
        &mut self,
        token: Address,
        to: Address,
        value: U256,
    ) -> bool {
        let result = self.only_owner().is_ok()
            && self.safe_erc20.try_safe_transfer(token, to, value);

        // log the result so we can assert it in tests.
        if result {
            self.vm().log(True {});
        } else {
            self.vm().log(False {});
        }
        result
    }

    fn try_safe_transfer_from(
        &mut self,
        token: Address,
        from: Address,
        to: Address,
        value: U256,
    ) -> bool {
        let result = self.only_owner().is_ok()
            && self.safe_erc20.try_safe_transfer_from(token, from, to, value);

        // log the result so we can assert it in tests.
        if result {
            self.vm().log(True {});
        } else {
            self.vm().log(False {});
        }
        result
    }

    fn safe_increase_allowance(
        &mut self,
        token: Address,
        spender: Address,
        value: U256,
    ) -> Result<(), Error> {
        self.only_owner()?;
        self.safe_erc20
            .safe_increase_allowance(token, spender, value)
            .map_err(Into::into)
    }

    fn safe_decrease_allowance(
        &mut self,
        token: Address,
        spender: Address,
        requested_decrease: U256,
    ) -> Result<(), Error> {
        self.only_owner()?;
        self.safe_erc20
            .safe_decrease_allowance(token, spender, requested_decrease)
            .map_err(Into::into)
    }

    fn force_approve(
        &mut self,
        token: Address,
        spender: Address,
        value: U256,
    ) -> Result<(), Error> {
        self.only_owner()?;
        self.safe_erc20.force_approve(token, spender, value).map_err(Into::into)
    }

    fn transfer_and_call_relaxed(
        &mut self,
        token: Address,
        to: Address,
        value: U256,
        data: Bytes,
    ) -> Result<(), Error> {
        self.only_owner()?;
        self.safe_erc20
            .transfer_and_call_relaxed(token, to, value, data)
            .map_err(Into::into)
    }

    fn transfer_from_and_call_relaxed(
        &mut self,
        token: Address,
        from: Address,
        to: Address,
        value: U256,
        data: Bytes,
    ) -> Result<(), Error> {
        self.only_owner()?;
        self.safe_erc20
            .transfer_from_and_call_relaxed(token, from, to, value, data)
            .map_err(Into::into)
    }

    fn approve_and_call_relaxed(
        &mut self,
        token: Address,
        spender: Address,
        value: U256,
        data: Bytes,
    ) -> Result<(), Error> {
        self.only_owner()?;
        self.safe_erc20
            .approve_and_call_relaxed(token, spender, value, data)
            .map_err(Into::into)
    }
}
