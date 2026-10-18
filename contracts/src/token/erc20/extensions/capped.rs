//! Capped Contract.
//!
//! Extension of ERC-20 standard that adds a cap to the supply of tokens.
//!
//! Note that they will not be capped by simply including this module,
//! but only once the checks are put in place.

use alloc::{vec, vec::Vec};

use alloy_primitives::U256;
use openzeppelin_stylus_proc::interface_id;
pub use sol::*;
use stylus_sdk::{prelude::*, storage::StorageU256};

#[cfg_attr(coverage_nightly, coverage(off))]
mod sol {
    use alloy_sol_macro::sol;

    sol! {
        /// Indicates an error related to the operation that failed
        /// because `total_supply` exceeded the `cap`.
        #[derive(Debug)]
        #[allow(missing_docs)]
        error ERC20ExceededCap(uint256 increased_supply, uint256 cap);

        /// Indicates an error related to the operation that failed
        /// because the supplied `cap` is not a valid cap value.
        #[derive(Debug)]
        #[allow(missing_docs)]
        error ERC20InvalidCap(uint256 cap);
    }
}

/// A Capped error.
#[derive(SolidityError, Debug)]
pub enum Error {
    /// Indicates an error related to the operation that failed
    /// because `total_supply` exceeded the `cap`.
    ExceededCap(ERC20ExceededCap),
    /// Indicates an error related to the operation that failed
    /// because the supplied `cap` is not a valid cap value.
    InvalidCap(ERC20InvalidCap),
}

impl MethodError for Error {
    fn encode(self) -> alloc::vec::Vec<u8> {
        self.into()
    }
}

/// State of a [`Capped`] Contract.
#[storage]
pub struct Capped {
    /// A cap to the supply of tokens.
    pub(crate) cap: StorageU256,
}

/// Interface for the token supply cap logic.
#[interface_id]
pub trait ICapped {
    /// Returns the cap on the token's total supply.
    #[must_use]
    fn cap(&self) -> U256;
}

#[public]
#[implements(ICapped)]
impl Capped {
    /// Constructor.
    ///
    /// # Arguments
    ///
    /// * `&mut self` - Write access to the contract's state.
    /// * `cap` - The token supply cap.
    ///
    /// # Errors
    ///
    /// * [`Error::InvalidCap`] - If cap is [`U256::ZERO`].
    #[constructor]
    pub fn constructor(&mut self, cap: U256) -> Result<(), Error> {
        if cap.is_zero() {
            return Err(Error::InvalidCap(ERC20InvalidCap { cap }));
        }
        self.cap.set(cap);
        Ok(())
    }
}

impl Capped {
    /// Checks whether increasing the current supply by `increase` stays
    /// within the configured cap.
    ///
    /// Call this before minting in the consuming contract. Including
    /// [`Capped`] does not automatically route an
    /// [`Erc20`](crate::token::erc20::Erc20) mint through this
    /// check.
    ///
    /// # Errors
    ///
    /// * [`Error::ExceededCap`] - If the resulting supply is greater than the
    ///   configured cap.
    pub fn _check_cap(
        &self,
        current_supply: U256,
        increase: U256,
    ) -> Result<(), Error> {
        let increased_supply = current_supply
            .checked_add(increase)
            .expect("increased supply should not overflow U256");
        let cap = self.cap.get();

        if increased_supply > cap {
            return Err(Error::ExceededCap(ERC20ExceededCap {
                increased_supply,
                cap,
            }));
        }

        Ok(())
    }
}

#[public]
impl ICapped for Capped {
    fn cap(&self) -> U256 {
        self.cap.get()
    }
}

use stylus_sdk::prelude::errors::MethodError;
#[cfg(test)]
mod tests {
    use crate::test_support::{test_address, TestContract as TestVMContract};
    use alloy_primitives::uint;
    use stylus_sdk::prelude::*;

    use super::*;

    unsafe impl TopLevelStorage for Capped {}

    #[test]
    fn cap_works() {
        let contract = TestVMContract::<Capped>::new(stringify!(contract));
        let alice = test_address(stringify!(alice));

        let value = uint!(2024_U256);
        contract.sender(alice).constructor(value).expect("should set cap");
        assert_eq!(contract.sender(alice).cap(), value);

        let value = uint!(1_U256);
        contract.sender(alice).constructor(value).expect("should set cap");
        assert_eq!(contract.sender(alice).cap(), value);

        let value = uint!(0_U256);
        let err = contract
            .sender(alice)
            .constructor(value)
            .expect_err("should return error");
        assert!(matches!(
            err,
            Error::InvalidCap(ERC20InvalidCap { cap }) if cap == value
        ));
    }

    #[test]
    fn check_cap_validates_proposed_supply() {
        let contract = TestVMContract::<Capped>::new(stringify!(contract));
        let alice = test_address(stringify!(alice));
        let cap = uint!(100_U256);

        contract.sender(alice).constructor(cap).expect("should set cap");
        contract
            .sender(alice)
            ._check_cap(uint!(40_U256), uint!(60_U256))
            .expect("supply at cap should be accepted");

        let err = contract
            .sender(alice)
            ._check_cap(uint!(40_U256), uint!(61_U256))
            .expect_err("supply over cap should be rejected");
        assert!(matches!(
            err,
            Error::ExceededCap(ERC20ExceededCap { increased_supply, cap })
                if increased_supply == uint!(101_U256) && cap == uint!(100_U256)
        ));
    }
}
