//! Optional Burnable extension of the ERC-20 standard.

use alloc::vec::Vec;

use alloy_primitives::{Address, U256};
use stylus_sdk::prelude::*;

use crate::token::erc20::{self, Erc20};

/// Extension of [`Erc20`] that allows token holders to destroy both
/// their own tokens and those that they have an allowance for,
/// in a way that can be recognized off-chain (via event analysis).
#[public]
pub trait IErc20Burnable {
    /// The error type associated to this ERC-20 Burnable trait implementation.
    type Error: Into<Vec<u8>>;

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

    fn burn(&mut self, value: U256) -> Result<(), Self::Error> {
        self._burn(self.vm().msg_sender(), value)
    }

    fn burn_from(
        &mut self,
        account: Address,
        value: U256,
    ) -> Result<(), Self::Error> {
        self._spend_allowance(account, self.vm().msg_sender(), value)?;
        self._burn(account, value)
    }
}

#[cfg(test)]
mod tests {
    use alloy_primitives::{uint, Address, U256};
    use stylus_sdk::testing::TestVM;

    use super::IErc20Burnable;
    use crate::token::erc20::{Erc20, Error, IErc20};

    #[test]
    fn burns() {
        let contract_vm = TestVM::default();
        let contract_address = Address::from([160u8; 20]);
        contract_vm.set_contract_address(contract_address);
        let mut contract = Erc20::from(&contract_vm);
        let alice = Address::from([1u8; 20]);

        let zero = U256::ZERO;
        let one = uint!(1_U256);

        assert_eq!(
            zero,
            ({
                contract_vm.set_sender(alice);
                &mut contract
            })
            .total_supply()
        );

        // Mint some tokens for Alice.

        let two = uint!(2_U256);
        ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        ._update(Address::ZERO, alice, two)
        .unwrap();
        assert_eq!(
            two,
            ({
                contract_vm.set_sender(alice);
                &mut contract
            })
            .balance_of(alice)
        );
        assert_eq!(
            two,
            ({
                contract_vm.set_sender(alice);
                &mut contract
            })
            .total_supply()
        );

        ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .burn(one)
        .unwrap();

        assert_eq!(
            one,
            ({
                contract_vm.set_sender(alice);
                &mut contract
            })
            .balance_of(alice)
        );
        assert_eq!(
            one,
            ({
                contract_vm.set_sender(alice);
                &mut contract
            })
            .total_supply()
        );
    }

    #[test]
    fn burns_errors_when_insufficient_balance() {
        let contract_vm = TestVM::default();
        let contract_address = Address::from([160u8; 20]);
        contract_vm.set_contract_address(contract_address);
        let mut contract = Erc20::from(&contract_vm);
        let alice = Address::from([1u8; 20]);

        let zero = U256::ZERO;
        let one = uint!(1_U256);

        assert_eq!(
            zero,
            ({
                contract_vm.set_sender(alice);
                &mut contract
            })
            .balance_of(alice)
        );

        let result = ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .burn(one);
        assert!(matches!(result, Err(Error::InsufficientBalance(_))));
    }

    #[test]
    fn burn_from() {
        let contract_vm = TestVM::default();
        let contract_address = Address::from([160u8; 20]);
        contract_vm.set_contract_address(contract_address);
        let mut contract = Erc20::from(&contract_vm);
        let alice = Address::from([1u8; 20]);
        let bob = Address::from([2u8; 20]);

        // Alice approves the caller.
        let one = uint!(1_U256);
        ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .approve(bob, one)
        .unwrap();

        // Mint some tokens for Alice.
        let two = uint!(2_U256);
        ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        ._update(Address::ZERO, alice, two)
        .unwrap();
        assert_eq!(
            two,
            ({
                contract_vm.set_sender(alice);
                &mut contract
            })
            .balance_of(alice)
        );
        assert_eq!(
            two,
            ({
                contract_vm.set_sender(alice);
                &mut contract
            })
            .total_supply()
        );

        ({
            contract_vm.set_sender(bob);
            &mut contract
        })
        .burn_from(alice, one)
        .unwrap();

        assert_eq!(
            one,
            ({
                contract_vm.set_sender(alice);
                &mut contract
            })
            .balance_of(alice)
        );
        assert_eq!(
            one,
            ({
                contract_vm.set_sender(alice);
                &mut contract
            })
            .total_supply()
        );
        assert_eq!(
            U256::ZERO,
            ({
                contract_vm.set_sender(alice);
                &mut contract
            })
            .allowance(bob, alice)
        );
    }

    #[test]
    fn burns_from_errors_when_insufficient_balance() {
        let contract_vm = TestVM::default();
        let contract_address = Address::from([160u8; 20]);
        contract_vm.set_contract_address(contract_address);
        let mut contract = Erc20::from(&contract_vm);
        let alice = Address::from([1u8; 20]);
        let bob = Address::from([2u8; 20]);

        // Alice approves the caller.
        let zero = U256::ZERO;
        let one = uint!(1_U256);

        ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .approve(bob, one)
        .unwrap();
        assert_eq!(
            zero,
            ({
                contract_vm.set_sender(alice);
                &mut contract
            })
            .balance_of(bob)
        );

        let one = uint!(1_U256);

        let result = ({
            contract_vm.set_sender(bob);
            &mut contract
        })
        .burn_from(alice, one);
        assert!(matches!(result, Err(Error::InsufficientBalance(_))));
    }

    #[test]
    fn burns_from_errors_when_insufficient_allowance() {
        let contract_vm = TestVM::default();
        let contract_address = Address::from([160u8; 20]);
        contract_vm.set_contract_address(contract_address);
        let mut contract = Erc20::from(&contract_vm);
        let alice = Address::from([1u8; 20]);

        // Mint some tokens for Alice.
        let one = uint!(1_U256);
        ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        ._update(Address::ZERO, alice, one)
        .unwrap();
        assert_eq!(
            one,
            ({
                contract_vm.set_sender(alice);
                &mut contract
            })
            .balance_of(alice)
        );

        let result = ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .burn_from(alice, one);
        assert!(matches!(result, Err(Error::InsufficientAllowance(_))));
    }
}
