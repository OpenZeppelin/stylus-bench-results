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
use stylus_sdk::{
    prelude::*, storage::StorageU256, stylus_core::calls::errors::MethodError,
};

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
#[public]
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
    /// Checks whether increasing the supply by `value` would exceed the cap.
    ///
    /// Call this from the mint path before updating the token supply. Keeping
    /// the policy check on the extension lets a composed token reuse the same
    /// cap behavior without duplicating its error construction.
    ///
    /// # Errors
    ///
    /// * [`Error::ExceededCap`] - If the increased supply exceeds the cap.
    ///
    /// # Panics
    ///
    /// * If the increased supply exceeds [`U256::MAX`].
    pub fn check_increase(
        &self,
        total_supply: U256,
        value: U256,
    ) -> Result<(), Error> {
        let increased_supply = total_supply
            .checked_add(value)
            .expect("increased supply should not exceed `U256::MAX`");
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

#[cfg(test)]
mod tests {
    use alloy_primitives::{uint, Address};
    use stylus_sdk::prelude::*;
    use stylus_sdk::testing::TestVM;

    use super::*;

    unsafe impl TopLevelStorage for Capped {}

    #[test]
    fn cap_works() {
        let contract_vm = TestVM::default();
        let contract_address = Address::from([160u8; 20]);
        contract_vm.set_contract_address(contract_address);
        let mut contract = Capped::from(&contract_vm);
        let alice = Address::from([1u8; 20]);

        let value = uint!(2024_U256);
        ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .constructor(value)
        .expect("should set cap");
        assert_eq!(
            ({
                contract_vm.set_sender(alice);
                &mut contract
            })
            .cap(),
            value
        );

        let value = uint!(1_U256);
        ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .constructor(value)
        .expect("should set cap");
        assert_eq!(
            ({
                contract_vm.set_sender(alice);
                &mut contract
            })
            .cap(),
            value
        );

        let value = uint!(0_U256);
        let err = ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .constructor(value)
        .expect_err("should return error");
        assert!(matches!(
            err,
            Error::InvalidCap(ERC20InvalidCap { cap }) if cap == value
        ));
    }

    #[test]
    fn check_increase_accepts_amounts_at_or_below_cap() {
        let contract_vm = TestVM::default();
        let mut capped = Capped::from(&contract_vm);
        let cap = uint!(100_U256);
        capped.cap.set(cap);

        assert!(capped.check_increase(uint!(99_U256), uint!(1_U256)).is_ok());
        assert!(capped.check_increase(cap, U256::ZERO).is_ok());
    }

    #[test]
    fn check_increase_returns_the_standard_exceeded_cap_error() {
        let contract_vm = TestVM::default();
        let mut capped = Capped::from(&contract_vm);
        let cap = uint!(100_U256);
        let total_supply = uint!(99_U256);
        let value = uint!(2_U256);
        capped.cap.set(cap);

        let err = capped
            .check_increase(total_supply, value)
            .expect_err("increasing beyond the cap should fail");

        assert!(matches!(
            err,
            Error::ExceededCap(ERC20ExceededCap { increased_supply, cap: actual_cap })
                if increased_supply == total_supply + value && actual_cap == cap
        ));
    }

    #[test]
    #[should_panic(expected = "increased supply should not exceed `U256::MAX`")]
    fn check_increase_panics_on_overflow() {
        let contract_vm = TestVM::default();
        let capped = Capped::from(&contract_vm);
        capped.check_increase(U256::MAX, uint!(1_U256)).unwrap();
    }
}
