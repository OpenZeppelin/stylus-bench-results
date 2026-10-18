//! Optional Burnable extension of the ERC-20 standard.

use alloy_primitives::{Address, U256};
use stylus_sdk::prelude::{HostAccess, MessageAccess};

use crate::token::erc20::{self, Erc20};

/// Extension of [`Erc20`] that allows token holders to destroy both
/// their own tokens and those that they have an allowance for,
/// in a way that can be recognized off-chain (via event analysis).
#[allow(missing_docs)]
#[stylus_sdk::prelude::public]
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

impl IErc20Burnable for Erc20 {
    #[cfg(not(feature = "contract-client-gen"))]
    fn __stylus_trait_and_impl_must_be_tagged_with_public_macro(&self) {}

    type Error = erc20::Error;

    fn burn(&mut self, value: U256) -> Result<(), erc20::Error> {
        self._burn(self.vm().msg_sender(), value)
    }

    fn burn_from(
        &mut self,
        account: Address,
        value: U256,
    ) -> Result<(), erc20::Error> {
        let spender = self.vm().msg_sender();
        let remaining_allowance =
            self._allowance_after_spending(account, spender, value)?;
        self._burn(account, value)?;
        self._write_spent_allowance(account, spender, remaining_allowance)
    }
}

#[cfg(test)]
mod tests {
    use crate::test_support::{TestContract as TestVMContract, test_address};
    use alloy_primitives::{Address, U256, uint};

    use super::IErc20Burnable;
    use crate::token::erc20::{Erc20, Error, IErc20};

    #[test]
    fn burns() {
        let contract = TestVMContract::<Erc20>::new(stringify!(contract));
        let alice = test_address(stringify!(alice));

        let zero = U256::ZERO;
        let one = uint!(1_U256);

        assert_eq!(zero, contract.sender(alice).total_supply());

        // Mint some tokens for Alice.

        let two = uint!(2_U256);
        contract.sender(alice)._update(Address::ZERO, alice, two).unwrap();
        assert_eq!(two, contract.sender(alice).balance_of(alice));
        assert_eq!(two, contract.sender(alice).total_supply());

        contract.sender(alice).burn(one).unwrap();

        assert_eq!(one, contract.sender(alice).balance_of(alice));
        assert_eq!(one, contract.sender(alice).total_supply());
    }

    #[test]
    fn burns_errors_when_insufficient_balance() {
        let contract = TestVMContract::<Erc20>::new(stringify!(contract));
        let alice = test_address(stringify!(alice));

        let zero = U256::ZERO;
        let one = uint!(1_U256);

        assert_eq!(zero, contract.sender(alice).balance_of(alice));

        let result = contract.sender(alice).burn(one);
        assert!(matches!(result, Err(Error::InsufficientBalance(_))));
    }

    #[test]
    fn burn_from() {
        let contract = TestVMContract::<Erc20>::new(stringify!(contract));
        let alice = test_address(stringify!(alice));
        let bob = test_address(stringify!(bob));

        // Alice approves the caller.
        let one = uint!(1_U256);
        contract.sender(alice).approve(bob, one).unwrap();

        // Mint some tokens for Alice.
        let two = uint!(2_U256);
        contract.sender(alice)._update(Address::ZERO, alice, two).unwrap();
        assert_eq!(two, contract.sender(alice).balance_of(alice));
        assert_eq!(two, contract.sender(alice).total_supply());

        contract.sender(bob).burn_from(alice, one).unwrap();

        assert_eq!(one, contract.sender(alice).balance_of(alice));
        assert_eq!(one, contract.sender(alice).total_supply());
        assert_eq!(U256::ZERO, contract.sender(alice).allowance(bob, alice));
    }

    #[test]
    fn burns_from_errors_when_insufficient_balance() {
        let contract = TestVMContract::<Erc20>::new(stringify!(contract));
        let alice = test_address(stringify!(alice));
        let bob = test_address(stringify!(bob));

        // Alice approves the caller.
        let zero = U256::ZERO;
        let one = uint!(1_U256);

        contract.sender(alice).approve(bob, one).unwrap();
        assert_eq!(zero, contract.sender(alice).balance_of(bob));

        let one = uint!(1_U256);

        let result = contract.sender(bob).burn_from(alice, one);
        assert!(matches!(result, Err(Error::InsufficientBalance(_))));
    }

    #[test]
    fn burns_from_errors_when_insufficient_allowance() {
        let contract = TestVMContract::<Erc20>::new(stringify!(contract));
        let alice = test_address(stringify!(alice));

        // Mint some tokens for Alice.
        let one = uint!(1_U256);
        contract.sender(alice)._update(Address::ZERO, alice, one).unwrap();
        assert_eq!(one, contract.sender(alice).balance_of(alice));

        let result = contract.sender(alice).burn_from(alice, one);
        assert!(matches!(result, Err(Error::InsufficientAllowance(_))));
    }
}
