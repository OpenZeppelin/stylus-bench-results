//! Optional Burnable extension of the ERC-20 standard.

use alloc::vec::Vec;

use alloy_primitives::{Address, U256};
use stylus_sdk::prelude::*;

use crate::token::erc20::{self, Erc20};

/// Extension of [`Erc20`] that allows token holders to destroy both
/// their own tokens and those that they have an allowance for,
/// in a way that can be recognized off-chain (via event analysis).
#[public]
// The `#[public]` macro adds an undocumented helper method to the trait.
#[allow(missing_docs)]
pub trait IErc20Burnable {
    /// The error type associated to this ERC-20 Burnable trait implementation.
    type Error: Into<alloc::vec::Vec<u8>>;

    /// Destroys a `value` amount of tokens from the caller, lowering the total
    /// supply.
    ///
    /// # Arguments
    ///
    /// * `&mut self` - Write access to the contract's state.
    /// * `value` - Amount to be burnt.
    ///
    /// # Errors
    ///
    /// * [`erc20::Error::InsufficientBalance`] - If the `from` address doesn't
    ///   have enough tokens.
    ///
    /// # Events
    ///
    /// * [`erc20::Transfer`].
    fn burn(&mut self, value: U256) -> Result<(), Self::Error>;

    /// Destroys a `value` amount of tokens from `account`, lowering the total
    /// supply.
    ///
    /// # Arguments
    ///
    /// * `&mut self` - Write access to the contract's state.
    /// * `account` - Owner's address.
    /// * `value` - Amount to be burnt.
    ///
    /// # Errors
    ///
    /// * [`erc20::Error::InsufficientAllowance`] - If not enough allowance is
    ///   available.
    /// * [`erc20::Error::InvalidSender`] - If the `from` address is
    ///   [`Address::ZERO`].
    /// * [`erc20::Error::InsufficientBalance`] - If the `from` address doesn't
    ///   have enough tokens.
    ///
    /// # Events
    ///
    /// * [`erc20::Transfer`].
    fn burn_from(
        &mut self,
        account: Address,
        value: U256,
    ) -> Result<(), Self::Error>;
}

#[public]
impl IErc20Burnable for Erc20 {
    type Error = erc20::Error;

    fn burn(&mut self, value: U256) -> Result<(), erc20::Error> {
        self._burn(self.vm().msg_sender(), value)
    }

    fn burn_from(
        &mut self,
        account: Address,
        value: U256,
    ) -> Result<(), erc20::Error> {
        self._spend_allowance(account, self.vm().msg_sender(), value)?;
        self._burn(account, value)
    }
}

#[cfg(test)]
mod tests {
    use alloy_primitives::{uint, Address, U256};
    use stylus_sdk::testing::TestVM;

    use super::IErc20Burnable;
    use crate::{
        test_utils::{ALICE, BOB},
        token::erc20::{Erc20, Error, IErc20},
    };

    #[test]
    fn burns() {
        let vm = TestVM::default();
        let mut contract = Erc20::from(&vm);

        let zero = U256::ZERO;
        let one = uint!(1_U256);

        assert_eq!(zero, contract.total_supply());

        // Mint some tokens for Alice.

        let two = uint!(2_U256);
        contract._update(Address::ZERO, ALICE, two).unwrap();
        assert_eq!(two, contract.balance_of(ALICE));
        assert_eq!(two, contract.total_supply());

        vm.set_sender(ALICE);
        contract.burn(one).unwrap();

        assert_eq!(one, contract.balance_of(ALICE));
        assert_eq!(one, contract.total_supply());
    }

    #[test]
    fn burns_errors_when_insufficient_balance() {
        let vm = TestVM::default();
        let mut contract = Erc20::from(&vm);

        let zero = U256::ZERO;
        let one = uint!(1_U256);

        assert_eq!(zero, contract.balance_of(ALICE));

        vm.set_sender(ALICE);
        let result = contract.burn(one);
        assert!(matches!(result, Err(Error::InsufficientBalance(_))));
    }

    #[test]
    fn burn_from() {
        let vm = TestVM::default();
        let mut contract = Erc20::from(&vm);

        // Alice approves Bob, who is the caller of `burn_from`.
        let one = uint!(1_U256);
        vm.set_sender(ALICE);
        contract.approve(BOB, one).unwrap();

        // Mint some tokens for Alice.
        let two = uint!(2_U256);
        contract._update(Address::ZERO, ALICE, two).unwrap();
        assert_eq!(two, contract.balance_of(ALICE));
        assert_eq!(two, contract.total_supply());

        vm.set_sender(BOB);
        contract.burn_from(ALICE, one).unwrap();

        assert_eq!(one, contract.balance_of(ALICE));
        assert_eq!(one, contract.total_supply());
        assert_eq!(U256::ZERO, contract.allowance(BOB, ALICE));
    }

    #[test]
    fn burns_from_errors_when_insufficient_balance() {
        let vm = TestVM::default();
        let mut contract = Erc20::from(&vm);

        // Alice approves Bob, who is the caller of `burn_from`.
        let zero = U256::ZERO;
        let one = uint!(1_U256);

        vm.set_sender(ALICE);
        contract.approve(BOB, one).unwrap();
        assert_eq!(zero, contract.balance_of(BOB));

        let one = uint!(1_U256);

        vm.set_sender(BOB);
        let result = contract.burn_from(ALICE, one);
        assert!(matches!(result, Err(Error::InsufficientBalance(_))));
    }

    #[test]
    fn burns_from_errors_when_insufficient_allowance() {
        let vm = TestVM::default();
        let mut contract = Erc20::from(&vm);

        // Mint some tokens for Alice.
        let one = uint!(1_U256);
        contract._update(Address::ZERO, ALICE, one).unwrap();
        assert_eq!(one, contract.balance_of(ALICE));

        vm.set_sender(ALICE);
        let result = contract.burn_from(ALICE, one);
        assert!(matches!(result, Err(Error::InsufficientAllowance(_))));
    }
}
