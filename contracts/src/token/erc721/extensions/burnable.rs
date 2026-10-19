//! Optional Burnable extension of the ERC-721 standard.
use alloc::vec::Vec;

use alloy_primitives::{Address, U256};
use openzeppelin_stylus_proc::interface_id;
use stylus_sdk::prelude::*;

use crate::token::erc721::{self, Erc721};

/// An [`Erc721`] token that can be burned (destroyed).
#[interface_id]
#[public]
// The `#[public]` macro adds an undocumented helper method to the trait.
#[allow(missing_docs)]
pub trait IErc721Burnable {
    /// The error type associated to this trait implementation.
    type Error: Into<alloc::vec::Vec<u8>>;

    /// Burns `token_id`.
    /// The approval is cleared when the token is burned.
    ///
    /// # Arguments
    ///
    /// * `value` - Amount to be burnt.
    ///
    /// # Errors
    ///
    /// * [`erc721::Error::NonexistentToken`] - If token does not exist.
    /// * [`erc721::Error::InsufficientApproval`] - If the caller does not have
    ///   the right to approve.
    ///
    /// # Events
    ///
    /// * [`erc721::Transfer`].
    fn burn(&mut self, token_id: U256) -> Result<(), Self::Error>;
}

#[public]
impl IErc721Burnable for Erc721 {
    type Error = erc721::Error;

    fn burn(&mut self, token_id: U256) -> Result<(), erc721::Error> {
        // Setting an "auth" arguments enables the
        // [`super::super::Erc721::_is_authorized`] check which verifies that
        // the token exists (from != [`Address::ZERO`]).
        //
        // Therefore, it is not needed to verify that the return value is not 0
        // here.
        self._update(Address::ZERO, token_id, self.vm().msg_sender())?;
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use alloy_primitives::uint;
    use stylus_sdk::testing::TestVM;

    use super::*;
    use crate::{
        test_utils::{ALICE, BOB},
        token::erc721::{
            ERC721InsufficientApproval, ERC721NonexistentToken, Erc721, Error,
            IErc721,
        },
    };

    const TOKEN_ID: U256 = uint!(1_U256);

    #[test]
    fn burns() {
        let vm = TestVM::default();
        let mut contract = Erc721::from(&vm);
        let one = uint!(1_U256);

        vm.set_sender(ALICE);
        contract._mint(ALICE, TOKEN_ID).expect("should mint a token for Alice");

        let initial_balance = contract
            .balance_of(ALICE)
            .expect("should return the balance of Alice");

        let result = contract.burn(TOKEN_ID);
        assert!(result.is_ok());

        let balance = contract
            .balance_of(ALICE)
            .expect("should return the balance of Alice");

        assert_eq!(initial_balance - one, balance);

        let err = contract
            .owner_of(TOKEN_ID)
            .expect_err("should return Error::NonexistentToken");

        assert!(matches!(
            err,
            Error::NonexistentToken (ERC721NonexistentToken{
                token_id: t_id
            }) if t_id == TOKEN_ID
        ));
    }

    #[test]
    fn burns_with_approval() {
        let vm = TestVM::default();
        let mut contract = Erc721::from(&vm);
        vm.set_sender(ALICE);
        contract._mint(BOB, TOKEN_ID).expect("should mint a token for Bob");

        let initial_balance =
            contract.balance_of(BOB).expect("should return the balance of Bob");

        vm.set_sender(BOB);
        contract
            .approve(ALICE, TOKEN_ID)
            .expect("should approve a token for Alice");

        vm.set_sender(ALICE);
        let result = contract.burn(TOKEN_ID);
        assert!(result.is_ok());

        let err = contract
            .owner_of(TOKEN_ID)
            .expect_err("should return Error::NonexistentToken");

        assert!(matches!(
            err,
            Error::NonexistentToken (ERC721NonexistentToken{
                token_id: t_id
            }) if t_id == TOKEN_ID
        ));

        let balance =
            contract.balance_of(BOB).expect("should return the balance of Bob");

        assert_eq!(initial_balance - uint!(1_U256), balance);
    }

    #[test]
    fn burns_with_approval_for_all() {
        let vm = TestVM::default();
        let mut contract = Erc721::from(&vm);
        vm.set_sender(ALICE);
        contract._mint(BOB, TOKEN_ID).expect("should mint a token for Bob");

        let initial_balance =
            contract.balance_of(BOB).expect("should return the balance of Bob");

        vm.set_sender(BOB);
        contract
            .set_approval_for_all(ALICE, true)
            .expect("should approve all Bob's tokens for Alice");

        vm.set_sender(ALICE);
        let result = contract.burn(TOKEN_ID);

        assert!(result.is_ok());

        let err = contract
            .owner_of(TOKEN_ID)
            .expect_err("should return Error::NonexistentToken");

        assert!(matches!(
            err,
            Error::NonexistentToken (ERC721NonexistentToken{
                token_id: t_id
            }) if t_id == TOKEN_ID
        ));

        let balance =
            contract.balance_of(BOB).expect("should return the balance of Bob");

        assert_eq!(initial_balance - uint!(1_U256), balance);
    }

    #[test]
    fn error_when_get_approved_of_previous_approval_burned() {
        let vm = TestVM::default();
        let mut contract = Erc721::from(&vm);
        vm.set_sender(ALICE);
        contract._mint(ALICE, TOKEN_ID).expect("should mint a token for Alice");
        contract
            .approve(BOB, TOKEN_ID)
            .expect("should approve a token for Bob");

        contract.burn(TOKEN_ID).expect("should burn previously minted token");

        let err = contract
            .get_approved(TOKEN_ID)
            .expect_err("should return Error::NonexistentToken");

        assert!(matches!(
            err,
            Error::NonexistentToken (ERC721NonexistentToken{
                token_id: t_id
            }) if t_id == TOKEN_ID
        ));
    }

    #[test]
    fn error_when_burn_without_approval() {
        let vm = TestVM::default();
        let mut contract = Erc721::from(&vm);
        vm.set_sender(ALICE);
        contract._mint(BOB, TOKEN_ID).expect("should mint a token for Bob");

        let err = contract
            .burn(TOKEN_ID)
            .expect_err("should not burn unapproved token");

        assert!(matches!(
            err,
            Error::InsufficientApproval(ERC721InsufficientApproval {
                    operator,
                    token_id: t_id,
            }) if operator == ALICE && t_id == TOKEN_ID
        ));
    }

    #[test]
    fn error_when_burn_nonexistent_token() {
        let vm = TestVM::default();
        let mut contract = Erc721::from(&vm);
        vm.set_sender(ALICE);
        let err = contract
            .burn(TOKEN_ID)
            .expect_err("should return Error::NonexistentToken");

        assert!(matches!(
            err,
            Error::NonexistentToken (ERC721NonexistentToken{
                token_id: t_id
            }) if t_id == TOKEN_ID
        ));
    }
}
