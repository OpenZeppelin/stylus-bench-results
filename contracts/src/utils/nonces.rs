//! Implementation of nonce tracking for addresses.
//!
//! Nonces will only increment.

use alloc::{vec, vec::Vec};

use alloy_primitives::{uint, Address, U256};
use stylus_sdk::{
    prelude::*,
    storage::{StorageMap, StorageU256},
    stylus_core::calls::errors::MethodError,
};

use crate::utils::math::storage::AddAssignChecked;

const ONE: U256 = uint!(1_U256);

pub use sol::*;
#[cfg_attr(coverage_nightly, coverage(off))]
mod sol {
    use alloy_sol_macro::sol;

    sol! {
        /// The nonce used for an `account` is not the expected current nonce.
        #[derive(Debug)]
        #[allow(missing_docs)]
        error InvalidAccountNonce(address account, uint256 current_nonce);
    }
}

/// A Nonces error.
#[derive(SolidityError, Debug)]
pub enum Error {
    /// The nonce used for an `account` is not the expected current nonce.
    InvalidAccountNonce(InvalidAccountNonce),
}

impl MethodError for Error {
    fn encode(self) -> alloc::vec::Vec<u8> {
        self.into()
    }
}

/// State of a [`Nonces`] Contract.
#[storage]
pub struct Nonces {
    /// Mapping from address to its nonce.
    pub(crate) nonces: StorageMap<Address, StorageU256>,
}

/// Interface for [`Nonces`]
#[public]
// The `#[public]` macro adds an undocumented helper method to the trait.
#[allow(missing_docs)]
pub trait INonces {
    /// Returns the unused nonce for the given account.
    ///
    /// # Arguments
    ///
    /// * `&self` - Read access to the contract's state.
    /// * `owner` - The address for which to return the nonce.
    #[must_use]
    fn nonces(&self, owner: Address) -> U256;
}

#[public]
#[implements(INonces)]
impl Nonces {}

#[public]
impl INonces for Nonces {
    fn nonces(&self, owner: Address) -> U256 {
        self.nonces.get(owner)
    }
}

impl Nonces {
    /// Consumes a nonce for the given `account`.
    ///
    /// # Arguments
    ///
    /// * `&mut self` - Write access to the contract's state.
    /// * `owner` - The address for which to consume the nonce.
    ///
    /// # Panics
    ///
    /// * If the nonce for the given `owner` exceeds [`U256::MAX`].
    pub fn use_nonce(&mut self, owner: Address) -> U256 {
        let nonce = self.nonces.get(owner);

        self.nonces
            .setter(owner)
            .add_assign_checked(ONE, "nonce should not exceed `U256::MAX`");

        nonce
    }

    /// Same as `use_nonce` but checking that the `nonce` is the next valid for
    /// the owner.
    ///
    /// # Arguments
    ///
    /// * `&mut self` - Write access to the contract's state.
    /// * `owner` - The address for which to consume the nonce.
    /// * `nonce` - The nonce to consume.
    ///
    /// # Errors
    ///
    /// * [`Error::InvalidAccountNonce`] - Returns an error if the `nonce` is
    ///   not the next valid nonce for the owner.
    ///
    /// # Panics
    ///
    /// * If the nonce for the given `owner` exceeds [`U256::MAX`].
    pub fn use_checked_nonce(
        &mut self,
        owner: Address,
        nonce: U256,
    ) -> Result<(), Error> {
        let current_nonce = self.use_nonce(owner);

        if nonce != current_nonce {
            return Err(Error::InvalidAccountNonce(InvalidAccountNonce {
                account: owner,
                current_nonce,
            }));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use stylus_sdk::testing::TestVM;

    use super::*;
    use crate::test_utils::ALICE;

    #[test]
    fn initiate_nonce() {
        let vm = TestVM::default();
        vm.set_sender(ALICE);
        let contract = Nonces::from(&vm);

        assert_eq!(contract.nonces(ALICE), U256::ZERO);
    }

    #[test]
    fn use_nonce() {
        let vm = TestVM::default();
        vm.set_sender(ALICE);
        let mut contract = Nonces::from(&vm);

        let use_nonce = contract.use_nonce(ALICE);
        assert_eq!(use_nonce, U256::ZERO);

        let nonce = contract.nonces(ALICE);
        assert_eq!(nonce, ONE);
    }

    #[test]
    fn use_checked_nonce() {
        let vm = TestVM::default();
        vm.set_sender(ALICE);
        let mut contract = Nonces::from(&vm);

        let use_checked_nonce = contract.use_checked_nonce(ALICE, U256::ZERO);
        assert!(use_checked_nonce.is_ok());

        let nonce = contract.nonces(ALICE);
        assert_eq!(nonce, ONE);
    }

    #[test]
    fn use_checked_nonce_invalid_nonce() {
        let vm = TestVM::default();
        vm.set_sender(ALICE);
        let mut contract = Nonces::from(&vm);

        let use_checked_nonce = contract.use_checked_nonce(ALICE, ONE);
        assert!(matches!(
            use_checked_nonce,
            Err(Error::InvalidAccountNonce(_))
        ));
    }
}
