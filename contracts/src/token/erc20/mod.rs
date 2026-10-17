//! Implementation of the ERC-20 token standard.
//!
//! We have followed general `OpenZeppelin` Contracts guidelines: functions
//! revert instead of returning `false` on failure. This behavior is
//! nonetheless conventional and does not conflict with the expectations of
//! [`Erc20`] applications.
use alloc::{vec, vec::Vec};

use alloy_primitives::{aliases::B32, Address, U256};
use openzeppelin_stylus_proc::interface_id;
use stylus_sdk::{
    prelude::*,
    storage::{StorageMap, StorageU256},
    stylus_core::calls::errors::MethodError,
};

use crate::utils::introspection::erc165::IErc165;

pub mod extensions;
pub mod interface;
pub mod utils;

pub use sol::*;
#[cfg_attr(coverage_nightly, coverage(off))]
mod sol {
    use alloy_sol_macro::sol;

    sol! {
        /// Emitted when `value` tokens are moved from one account (`from`) to
        /// another (`to`).
        ///
        /// Note that `value` may be zero.
        #[derive(Debug)]
        #[allow(missing_docs)]
        event Transfer(address indexed from, address indexed to, uint256 value);
        /// Emitted when the allowance of a `spender` for an `owner` is set by a
        /// call to `approve`. `value` is the new allowance.
        #[derive(Debug)]
        #[allow(missing_docs)]
        event Approval(address indexed owner, address indexed spender, uint256 value);
    }

    sol! {
        /// Indicates an error related to the current `balance` of `sender`. Used
        /// in transfers.
        ///
        /// * `sender` - Address whose tokens are being transferred.
        /// * `balance` - Current balance for the interacting account.
        /// * `needed` - Minimum amount required to perform a transfer.
        #[derive(Debug)]
        #[allow(missing_docs)]
        error ERC20InsufficientBalance(address sender, uint256 balance, uint256 needed);
        /// Indicates a failure with the token `sender`. Used in transfers.
        ///
        /// * `sender` - Address whose tokens are being transferred.
        #[derive(Debug)]
        #[allow(missing_docs)]
        error ERC20InvalidSender(address sender);
        /// Indicates a failure with the token `receiver`. Used in transfers.
        ///
        /// * `receiver` - Address to which the tokens are being transferred.
        #[derive(Debug)]
        #[allow(missing_docs)]
        error ERC20InvalidReceiver(address receiver);
        /// Indicates a failure with the `spender`’s `allowance`. Used in
        /// transfers.
        ///
        /// * `spender` - Address that may be allowed to operate on tokens without
        /// being their owner.
        /// * `allowance` - Amount of tokens a `spender` is allowed to operate
        /// with.
        /// * `needed` - Minimum amount required to perform a transfer.
        #[derive(Debug)]
        #[allow(missing_docs)]
        error ERC20InsufficientAllowance(address spender, uint256 allowance, uint256 needed);
        /// Indicates a failure with the `spender` to be approved. Used in
        /// approvals.
        ///
        /// * `spender` - Address that may be allowed to operate on tokens without
        /// being their owner.
        #[derive(Debug)]
        #[allow(missing_docs)]
        error ERC20InvalidSpender(address spender);

        /// Indicates a failure with the `approver` of a token to be approved. Used in approvals.
        /// approver Address initiating an approval operation.
        ///
        /// * `approver` - Address initiating an approval operation.
        #[derive(Debug)]
        #[allow(missing_docs)]
        error ERC20InvalidApprover(address approver);

    }
}

/// An [`Erc20`] error defined as described in [ERC-6093].
///
/// [ERC-6093]: https://eips.ethereum.org/EIPS/eip-6093
#[derive(SolidityError, Debug)]
pub enum Error {
    /// Indicates an error related to the current balance of `sender`. Used in
    /// transfers.
    InsufficientBalance(ERC20InsufficientBalance),
    /// Indicates a failure with the token `sender`. Used in transfers.
    InvalidSender(ERC20InvalidSender),
    /// Indicates a failure with the token `receiver`. Used in transfers.
    InvalidReceiver(ERC20InvalidReceiver),
    /// Indicates a failure with the `spender`’s `allowance`. Used in
    /// transfers.
    InsufficientAllowance(ERC20InsufficientAllowance),
    /// Indicates a failure with the `spender` to be approved. Used in
    /// approvals.
    InvalidSpender(ERC20InvalidSpender),
    /// Indicates a failure with the `approver` of a token to be approved. Used
    /// in approvals. approver Address initiating an approval operation.
    InvalidApprover(ERC20InvalidApprover),
}

impl MethodError for Error {
    fn encode(self) -> alloc::vec::Vec<u8> {
        self.into()
    }
}

/// State of an [`Erc20`] token.
#[storage]
pub struct Erc20 {
    /// Maps users to balances.
    pub(crate) balances: StorageMap<Address, StorageU256>,
    /// Maps users to a mapping of each spender's allowance.
    pub(crate) allowances:
        StorageMap<Address, StorageMap<Address, StorageU256>>,
    /// The total supply of the token.
    pub(crate) total_supply: StorageU256,
}

/// NOTE: Implementation of [`TopLevelStorage`] to be able use `&mut self` when
/// calling other contracts and not `&mut (impl TopLevelStorage +
/// BorrowMut<Self>)`. Should be fixed in the future by the Stylus team.
unsafe impl TopLevelStorage for Erc20 {}

/// Required interface of an [`Erc20`] compliant contract.
#[interface_id]
#[public]
pub trait IErc20 {
    /// The error type associated to this ERC-20 trait implementation.
    type Error: Into<alloc::vec::Vec<u8>>;

    /// Returns the number of tokens in existence.
    ///
    /// # Arguments
    ///
    /// * `&self` - Read access to the contract's state.
    fn total_supply(&self) -> U256;

    /// Returns the number of tokens owned by `account`.
    ///
    /// # Arguments
    ///
    /// * `&self` - Read access to the contract's state.
    /// * `account` - Account to get balance from.
    fn balance_of(&self, account: Address) -> U256;

    /// Moves a `value` amount of tokens from the caller's account to `to`.
    ///
    /// Returns a boolean value indicating whether the operation succeeded.
    ///
    /// # Arguments
    ///
    /// * `&mut self` - Write access to the contract's state.
    /// * `to` - Account to transfer tokens to.
    /// * `value` - Number of tokens to transfer.
    ///
    /// # Errors
    ///
    /// * [`Error::InvalidReceiver`] - If the `to` address is [`Address::ZERO`].
    /// * [`Error::InsufficientBalance`] - If the caller doesn't have a balance
    ///   of at least `value`.
    ///
    /// # Events
    ///
    /// * [`Transfer`].
    fn transfer(
        &mut self,
        to: Address,
        value: U256,
    ) -> Result<bool, Self::Error>;

    /// Returns the remaining number of tokens that `spender` will be allowed
    /// to spend on behalf of `owner` through `transfer_from`. This is zero by
    /// default.
    ///
    /// This value changes when `approve` or `transfer_from` are called.
    ///
    /// # Arguments
    ///
    /// * `&self` - Read access to the contract's state.
    /// * `owner` - Account that owns the tokens.
    /// * `spender` - Account that will spend the tokens.
    fn allowance(&self, owner: Address, spender: Address) -> U256;

    /// Sets a `value` number of tokens as the allowance of `spender` over the
    /// caller's tokens.
    ///
    /// Returns a boolean value indicating whether the operation succeeded.
    ///
    /// WARNING: Beware that changing an allowance with this method brings the
    /// risk that someone may use both the old and the new allowance by
    /// unfortunate transaction ordering. One possible solution to mitigate
    /// this race condition is to first reduce the `spender`'s allowance to 0
    /// and set the desired value afterwards:
    /// <https://github.com/ethereum/EIPs/issues/20#issuecomment-263524729>
    ///
    /// # Arguments
    ///
    /// * `&mut self` - Write access to the contract's state.
    /// * `spender` - Account that will spend the tokens.
    /// * `value` - Number of tokens the spender is allowed to spend.
    ///
    /// # Errors
    ///
    /// * [`Error::InvalidSpender`] - If the `spender` address is
    ///   [`Address::ZERO`].
    ///
    /// # Events
    ///
    /// * [`Approval`].
    fn approve(
        &mut self,
        spender: Address,
        value: U256,
    ) -> Result<bool, Self::Error>;

    /// Moves a `value` number of tokens from `from` to `to` using the
    /// allowance mechanism. `value` is then deducted from the caller's
    /// allowance.
    ///
    /// Returns a boolean value indicating whether the operation succeeded.
    ///
    /// NOTE: If `value` is the maximum [`U256::MAX`], the allowance is not
    /// updated on `transfer_from`. This is semantically equivalent to
    /// an infinite approval.
    ///
    /// # Arguments
    ///
    /// * `&mut self` - Write access to the contract's state.
    /// * `from` - Account to transfer tokens from.
    /// * `to` - Account to transfer tokens to.
    /// * `value` - Number of tokens to transfer.
    ///
    /// # Errors
    ///
    /// * [`Error::InvalidSender`] - If the `from` address is [`Address::ZERO`].
    /// * [`Error::InvalidReceiver`] - If the `to` address is [`Address::ZERO`].
    /// * [`Error::InsufficientAllowance`] - If not enough allowance is
    ///   available.
    /// * [`Error::InsufficientBalance`] - If the `from` address doesn't have
    ///   enough tokens, then the error
    ///  is returned.
    ///
    /// # Events
    ///
    /// * [`Transfer`].
    fn transfer_from(
        &mut self,
        from: Address,
        to: Address,
        value: U256,
    ) -> Result<bool, Self::Error>;
}

#[public]
#[implements(IErc20<Error = Error>)]
impl Erc20 {}

#[public]
impl IErc20 for Erc20 {
    type Error = Error;

    fn total_supply(&self) -> U256 {
        self.total_supply.get()
    }

    fn balance_of(&self, account: Address) -> U256 {
        self.balances.get(account)
    }

    fn transfer(
        &mut self,
        to: Address,
        value: U256,
    ) -> Result<bool, Self::Error> {
        let from = self.vm().msg_sender();
        self._transfer(from, to, value)?;
        Ok(true)
    }

    fn allowance(&self, owner: Address, spender: Address) -> U256 {
        self.allowances.get(owner).get(spender)
    }

    fn approve(
        &mut self,
        spender: Address,
        value: U256,
    ) -> Result<bool, Self::Error> {
        let owner = self.vm().msg_sender();
        self._approve(owner, spender, value, true)
    }

    fn transfer_from(
        &mut self,
        from: Address,
        to: Address,
        value: U256,
    ) -> Result<bool, Self::Error> {
        let spender = self.vm().msg_sender();
        let allowance_after_spend =
            self._allowance_after_spend(from, spender, value)?;
        self._transfer(from, to, value)?;
        if let Some(allowance) = allowance_after_spend {
            self.allowances.setter(from).insert(spender, allowance);
        }
        Ok(true)
    }
}

impl Erc20 {
    /// Sets a `value` number of tokens as the allowance of `spender` over the
    /// caller's tokens.
    ///
    /// Returns a boolean value indicating whether the operation succeeded.
    /// # Arguments
    ///
    /// * `&mut self` - Write access to the contract's state.
    /// * `owner` - Account that owns the tokens.
    /// * `spender` - Account that will spend the tokens.
    /// * `emit_event` - Emit an [`Approval`] event flag.
    ///
    /// # Errors
    ///
    /// * [`Error::InvalidSpender`] - If the `spender` address is
    ///   [`Address::ZERO`].
    ///
    /// # Events
    ///
    /// * [`Approval`].
    fn _approve(
        &mut self,
        owner: Address,
        spender: Address,
        value: U256,
        emit_event: bool,
    ) -> Result<bool, Error> {
        if owner.is_zero() {
            return Err(Error::InvalidApprover(ERC20InvalidApprover {
                approver: Address::ZERO,
            }));
        }

        if spender.is_zero() {
            return Err(Error::InvalidSpender(ERC20InvalidSpender {
                spender: Address::ZERO,
            }));
        }

        self.allowances.setter(owner).insert(spender, value);
        if emit_event {
            self.vm().log(Approval { owner, spender, value });
        }
        Ok(true)
    }

    /// Internal implementation of transferring tokens between two accounts.
    ///
    /// # Arguments
    ///
    /// * `&mut self` - Write access to the contract's state.
    /// * `from` - Account to transfer tokens from.
    /// * `to` - Account to transfer tokens to.
    /// * `value` - The number of tokens to transfer.
    ///
    /// # Errors
    ///
    /// * [`Error::InvalidSender`] - If the `from` address is [`Address::ZERO`].
    /// * [`Error::InvalidReceiver`] - If the `to` address is [`Address::ZERO`].
    /// * [`Error::InsufficientBalance`] - If the `from` address doesn't have
    ///   enough tokens.
    ///
    /// # Events
    ///
    /// * [`Transfer`].
    fn _transfer(
        &mut self,
        from: Address,
        to: Address,
        value: U256,
    ) -> Result<(), Error> {
        if from.is_zero() {
            return Err(Error::InvalidSender(ERC20InvalidSender {
                sender: Address::ZERO,
            }));
        }
        if to.is_zero() {
            return Err(Error::InvalidReceiver(ERC20InvalidReceiver {
                receiver: Address::ZERO,
            }));
        }

        self._update(from, to, value)?;

        Ok(())
    }

    /// Creates a `value` amount of tokens and assigns them to `account`,
    /// by transferring it from [`Address::ZERO`].
    ///
    /// # Errors
    ///
    /// * [`Error::InvalidReceiver`] - If the `account` address is
    ///   [`Address::ZERO`].
    ///
    /// # Events
    ///
    /// * [`Transfer`].
    ///
    /// # Panics
    ///
    /// * If `total_supply` exceeds [`U256::MAX`].
    pub fn _mint(
        &mut self,
        account: Address,
        value: U256,
    ) -> Result<(), Error> {
        if account.is_zero() {
            return Err(Error::InvalidReceiver(ERC20InvalidReceiver {
                receiver: Address::ZERO,
            }));
        }
        self._update(Address::ZERO, account, value)
    }

    /// Transfers a `value` amount of tokens from `from` to `to`, or
    /// alternatively mints (or burns) if `from` (or `to`) is the zero address.
    ///
    /// All customizations to transfers, mints, and burns should be done by
    /// using this function.
    ///
    /// # Arguments
    ///
    /// * `from` - Owner's address.
    /// * `to` - Recipient's address.
    /// * `value` - Amount to be transferred.
    ///
    /// # Errors
    ///
    /// * [`Error::InsufficientBalance`] - If the `from` address doesn't have
    ///   enough tokens.
    ///
    /// # Events
    ///
    /// * [`Transfer`].
    ///
    /// # Panics
    ///
    /// * If `total_supply` exceeds [`U256::MAX`]. It may happen during `mint`
    ///   operation.
    pub fn _update(
        &mut self,
        from: Address,
        to: Address,
        value: U256,
    ) -> Result<(), Error> {
        if from.is_zero() {
            // Mint operation. Overflow check required: the rest of the code
            // assumes that `total_supply` never overflows.
            self.total_supply
                .update_check_add(value)
                .expect("should not exceed U256::MAX for total_supply");
        } else {
            let from_balance = self.balances.get(from);
            if from_balance < value {
                return Err(Error::InsufficientBalance(
                    ERC20InsufficientBalance {
                        sender: from,
                        balance: from_balance,
                        needed: value,
                    },
                ));
            }
            // Overflow not possible:
            // `value` <= `from_balance` <= `total_supply`.
            self.balances.setter(from).set(from_balance - value);
        }

        if to.is_zero() {
            // Overflow not possible:
            // `value` <= `total_supply` or
            // `value` <= `from_balance` <= `total_supply`.
            self.total_supply.update_wrap_sub(value);
        } else {
            // Overflow not possible:
            // `balance_to` + `value` is at most `total_supply`,
            // which fits into a `U256`.
            self.balances.setter(to).update_wrap_add(value);
        }

        self.vm().log(Transfer { from, to, value });

        Ok(())
    }

    /// Destroys a `value` amount of tokens from `account`,
    /// lowering the total supply.
    ///
    /// # Arguments
    ///
    /// * `account` - Owner's address.
    /// * `value` - Amount to be burnt.
    ///
    /// # Errors
    ///
    /// * [`Error::InvalidSender`] - If the `from` address is [`Address::ZERO`].
    /// * [`Error::InsufficientBalance`] - If the `from` address doesn't have
    ///   enough tokens.
    ///
    /// # Events
    ///
    /// * [`Transfer`].
    pub fn _burn(
        &mut self,
        account: Address,
        value: U256,
    ) -> Result<(), Error> {
        if account == Address::ZERO {
            return Err(Error::InvalidSender(ERC20InvalidSender {
                sender: Address::ZERO,
            }));
        }
        self._update(account, Address::ZERO, value)
    }

    /// Updates `owner`'s allowance for `spender` based on spent `value`.
    ///
    /// Does not update the allowance value in the case of infinite allowance.
    ///
    /// # Arguments
    ///
    /// * `&mut self` - Write access to the contract's state.
    /// * `owner` - Account to transfer tokens from.
    /// * `to` - Account to transfer tokens to.
    /// * `value` - The number of tokens to transfer.
    ///
    /// # Errors
    ///
    /// * [`Error::InsufficientAllowance`] - If not enough allowance is
    ///   available.
    ///
    /// # Events
    ///
    /// * [`Approval`].
    pub fn _spend_allowance(
        &mut self,
        owner: Address,
        spender: Address,
        value: U256,
    ) -> Result<(), Error> {
        if let Some(allowance) =
            self._allowance_after_spend(owner, spender, value)?
        {
            self.allowances.setter(owner).insert(spender, allowance);
        }

        Ok(())
    }

    /// Returns the finite allowance remaining after a spend, validating the
    /// same conditions as `_spend_allowance` without changing storage.
    fn _allowance_after_spend(
        &self,
        owner: Address,
        spender: Address,
        value: U256,
    ) -> Result<Option<U256>, Error> {
        let current_allowance = self.allowance(owner, spender);
        if current_allowance == U256::MAX {
            return Ok(None);
        }
        if current_allowance < value {
            return Err(Error::InsufficientAllowance(
                ERC20InsufficientAllowance {
                    spender,
                    allowance: current_allowance,
                    needed: value,
                },
            ));
        }
        if owner.is_zero() {
            return Err(Error::InvalidApprover(ERC20InvalidApprover {
                approver: Address::ZERO,
            }));
        }
        if spender.is_zero() {
            return Err(Error::InvalidSpender(ERC20InvalidSpender {
                spender: Address::ZERO,
            }));
        }

        Ok(Some(current_allowance - value))
    }
}

#[public]
impl IErc165 for Erc20 {
    fn supports_interface(&self, interface_id: B32) -> bool {
        <Self as IErc20>::interface_id() == interface_id
            || <Self as IErc165>::interface_id() == interface_id
    }
}

#[cfg(test)]
mod tests {
    use alloy_primitives::{uint, Address, U256};
    use stylus_sdk::testing::TestVM;

    use super::*;

    use crate::test_utils::assert_emitted;

    #[test]
    fn mint() {
        let contract_vm = TestVM::default();
        let contract_address = Address::from([160u8; 20]);
        contract_vm.set_contract_address(contract_address);
        let mut contract = Erc20::from(&contract_vm);
        let alice = Address::from([1u8; 20]);

        let one = uint!(1_U256);

        // Store initial balance & supply.
        let initial_balance = ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .balance_of(alice);
        let initial_supply = ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .total_supply();

        // Mint action should work.
        let result = ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        ._mint(alice, one);
        assert!(result.is_ok());

        // Check updated balance & supply.
        assert_eq!(
            initial_balance + one,
            ({
                contract_vm.set_sender(alice);
                &mut contract
            })
            .balance_of(alice)
        );
        assert_eq!(
            initial_supply + one,
            ({
                contract_vm.set_sender(alice);
                &mut contract
            })
            .total_supply()
        );

        assert_emitted(
            &contract_vm,
            &Transfer { from: Address::ZERO, to: alice, value: one },
        );
    }

    #[test]
    #[should_panic = "should not exceed U256::MAX for total_supply"]
    fn update_mint_errors_arithmetic_overflow() {
        let contract_vm = TestVM::default();
        let contract_address = Address::from([160u8; 20]);
        contract_vm.set_contract_address(contract_address);
        let mut contract = Erc20::from(&contract_vm);
        let alice = Address::from([1u8; 20]);

        let one = uint!(1_U256);
        assert_eq!(
            U256::ZERO,
            ({
                contract_vm.set_sender(alice);
                &mut contract
            })
            .balance_of(alice)
        );
        assert_eq!(
            U256::ZERO,
            ({
                contract_vm.set_sender(alice);
                &mut contract
            })
            .total_supply()
        );

        // Initialize state for the test case:
        // Alice's balance as `U256::MAX`.
        ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        ._mint(alice, U256::MAX)
        .expect("should mint tokens");
        // Mint action should NOT work:
        // overflow on `total_supply`.
        let _result = ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        ._mint(alice, one);
    }

    #[test]
    fn mint_errors_invalid_receiver() {
        let contract_vm = TestVM::default();
        let contract_address = Address::from([160u8; 20]);
        contract_vm.set_contract_address(contract_address);
        let mut contract = Erc20::from(&contract_vm);
        let alice = Address::from([1u8; 20]);

        let receiver = Address::ZERO;
        let one = uint!(1_U256);

        // Store initial balance & supply.
        let initial_balance = ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .balance_of(receiver);
        let initial_supply = ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .total_supply();

        // Mint action should work.
        let err = ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        ._mint(receiver, one)
        .unwrap_err();
        assert!(matches!(err, Error::InvalidReceiver(_)));

        // Check updated balance & supply.
        assert_eq!(
            initial_balance,
            ({
                contract_vm.set_sender(alice);
                &mut contract
            })
            .balance_of(receiver)
        );
        assert_eq!(
            initial_supply,
            ({
                contract_vm.set_sender(alice);
                &mut contract
            })
            .total_supply()
        );
    }

    #[test]
    #[should_panic = "should not exceed U256::MAX for total_supply"]
    fn mint_errors_arithmetic_overflow() {
        let contract_vm = TestVM::default();
        let contract_address = Address::from([160u8; 20]);
        contract_vm.set_contract_address(contract_address);
        let mut contract = Erc20::from(&contract_vm);
        let alice = Address::from([1u8; 20]);

        let one = uint!(1_U256);
        assert_eq!(
            U256::ZERO,
            ({
                contract_vm.set_sender(alice);
                &mut contract
            })
            .balance_of(alice)
        );
        assert_eq!(
            U256::ZERO,
            ({
                contract_vm.set_sender(alice);
                &mut contract
            })
            .total_supply()
        );

        // Initialize state for the test case:
        // Alice's balance as [`U256::MAX`].
        ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        ._mint(alice, U256::MAX)
        .expect("should mint tokens");
        // Mint action should NOT work -- overflow on `_total_supply`.
        let _result = ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        ._mint(alice, one);
    }

    #[test]
    fn burn() {
        let contract_vm = TestVM::default();
        let contract_address = Address::from([160u8; 20]);
        contract_vm.set_contract_address(contract_address);
        let mut contract = Erc20::from(&contract_vm);
        let alice = Address::from([1u8; 20]);

        let one = uint!(1_U256);
        let two = uint!(2_U256);

        // Initialize state for the test case:
        // Alice's balance as `two`.
        ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        ._mint(alice, two)
        .expect("should mint tokens");

        // Store initial balance & supply.
        let initial_balance = ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .balance_of(alice);
        let initial_supply = ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .total_supply();

        // Burn action should work.
        ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        ._burn(alice, one)
        .unwrap();

        // Check updated balance & supply.
        assert_eq!(
            initial_balance - one,
            ({
                contract_vm.set_sender(alice);
                &mut contract
            })
            .balance_of(alice)
        );
        assert_eq!(
            initial_supply - one,
            ({
                contract_vm.set_sender(alice);
                &mut contract
            })
            .total_supply()
        );

        assert_emitted(
            &contract_vm,
            &Transfer { from: alice, to: Address::ZERO, value: one },
        );
    }

    #[test]
    fn burn_errors_insufficient_balance() {
        let contract_vm = TestVM::default();
        let contract_address = Address::from([160u8; 20]);
        contract_vm.set_contract_address(contract_address);
        let mut contract = Erc20::from(&contract_vm);
        let alice = Address::from([1u8; 20]);

        let one = uint!(1_U256);
        let two = uint!(2_U256);

        // Initialize state for the test case:
        // Alice's balance as `one`.
        ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        ._mint(alice, one)
        .expect("should mint tokens");

        // Store initial balance & supply.
        let initial_balance = ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .balance_of(alice);
        let initial_supply = ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .total_supply();

        // Burn action should NOT work - `InsufficientBalance`.
        let err = ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        ._burn(alice, two)
        .unwrap_err();
        assert!(matches!(err, Error::InsufficientBalance(_)));

        // Check proper state (before revert).
        assert_eq!(
            initial_balance,
            ({
                contract_vm.set_sender(alice);
                &mut contract
            })
            .balance_of(alice)
        );
        assert_eq!(
            initial_supply,
            ({
                contract_vm.set_sender(alice);
                &mut contract
            })
            .total_supply()
        );
    }

    #[test]
    fn transfer() {
        let contract_vm = TestVM::default();
        let contract_address = Address::from([160u8; 20]);
        contract_vm.set_contract_address(contract_address);
        let mut contract = Erc20::from(&contract_vm);
        let alice = Address::from([1u8; 20]);
        let bob = Address::from([2u8; 20]);

        let one = uint!(1_U256);

        // Initialize state for the test case:
        //  Alice's & Bob's balance as `one`.
        ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        ._mint(alice, one)
        .expect("should mint tokens");
        ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        ._mint(bob, one)
        .expect("should mint tokens");

        // Store initial balance & supply.
        let initial_alice_balance = ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .balance_of(alice);
        let initial_bob_balance = ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .balance_of(bob);
        let initial_supply = ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .total_supply();

        // Transfer action should work.
        let result = ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .transfer(bob, one);
        assert!(result.is_ok());

        // Check updated balance & supply.
        assert_eq!(
            initial_alice_balance - one,
            ({
                contract_vm.set_sender(alice);
                &mut contract
            })
            .balance_of(alice)
        );
        assert_eq!(
            initial_bob_balance + one,
            ({
                contract_vm.set_sender(alice);
                &mut contract
            })
            .balance_of(bob)
        );
        assert_eq!(
            initial_supply,
            ({
                contract_vm.set_sender(alice);
                &mut contract
            })
            .total_supply()
        );

        assert_emitted(
            &contract_vm,
            &Transfer { from: alice, to: bob, value: one },
        );
    }

    #[test]
    fn transfer_errors_insufficient_balance() {
        let contract_vm = TestVM::default();
        let contract_address = Address::from([160u8; 20]);
        contract_vm.set_contract_address(contract_address);
        let mut contract = Erc20::from(&contract_vm);
        let alice = Address::from([1u8; 20]);
        let bob = Address::from([2u8; 20]);

        let one = uint!(1_U256);

        // Initialize state for the test case:
        // Alice's & Bob's balance as `one`.
        ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        ._mint(alice, one)
        .expect("should mint tokens");
        ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        ._mint(bob, one)
        .expect("should mint tokens");

        // Store initial balance & supply.
        let initial_alice_balance = ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .balance_of(alice);
        let initial_bob_balance = ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .balance_of(bob);
        let initial_supply = ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .total_supply();

        // Transfer action should NOT work - `InsufficientBalance`.
        let err = ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .transfer(bob, one + one)
        .unwrap_err();
        assert!(matches!(err, Error::InsufficientBalance(_)));

        // Check proper state (before revert).
        assert_eq!(
            initial_alice_balance,
            ({
                contract_vm.set_sender(alice);
                &mut contract
            })
            .balance_of(alice)
        );
        assert_eq!(
            initial_bob_balance,
            ({
                contract_vm.set_sender(alice);
                &mut contract
            })
            .balance_of(bob)
        );
        assert_eq!(
            initial_supply,
            ({
                contract_vm.set_sender(alice);
                &mut contract
            })
            .total_supply()
        );
    }

    #[test]
    fn transfer_from() {
        let contract_vm = TestVM::default();
        let contract_address = Address::from([160u8; 20]);
        contract_vm.set_contract_address(contract_address);
        let mut contract = Erc20::from(&contract_vm);
        let alice = Address::from([1u8; 20]);
        let bob = Address::from([2u8; 20]);

        // Alice approves Bob.
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
        ._mint(alice, two)
        .unwrap();
        assert_eq!(
            two,
            ({
                contract_vm.set_sender(alice);
                &mut contract
            })
            .balance_of(alice)
        );

        ({
            contract_vm.set_sender(bob);
            &mut contract
        })
        .transfer_from(alice, bob, one)
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
            .balance_of(bob)
        );
        assert_eq!(
            U256::ZERO,
            ({
                contract_vm.set_sender(alice);
                &mut contract
            })
            .allowance(alice, bob)
        );

        assert_emitted(
            &contract_vm,
            &Transfer { from: alice, to: bob, value: one },
        );
    }

    #[test]
    fn error_when_transfer_with_insufficient_balance() {
        let contract_vm = TestVM::default();
        let contract_address = Address::from([160u8; 20]);
        contract_vm.set_contract_address(contract_address);
        let mut contract = Erc20::from(&contract_vm);
        let alice = Address::from([1u8; 20]);
        let bob = Address::from([2u8; 20]);

        // Alice approves Bob.
        let one = uint!(1_U256);
        ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .approve(bob, one)
        .unwrap();

        let err = ({
            contract_vm.set_sender(bob);
            &mut contract
        })
        .transfer_from(alice, bob, one)
        .unwrap_err();
        assert!(matches!(err, Error::InsufficientBalance(_)));
    }

    #[test]
    fn transfer_from_invalid_receiver_does_not_spend_allowance() {
        let contract_vm = TestVM::default();
        let contract_address = Address::from([160u8; 20]);
        contract_vm.set_contract_address(contract_address);
        let mut contract = Erc20::from(&contract_vm);
        let alice = Address::from([1u8; 20]);
        let bob = Address::from([2u8; 20]);
        let one = uint!(1_U256);
        let two = uint!(2_U256);

        ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        ._mint(alice, two)
        .unwrap();

        let err = ({
            contract_vm.set_sender(bob);
            &mut contract
        })
        .transfer_from(alice, Address::ZERO, one)
        .unwrap_err();
        assert!(matches!(err, Error::InsufficientAllowance(_)));

        ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .approve(bob, two)
        .unwrap();

        let err = ({
            contract_vm.set_sender(bob);
            &mut contract
        })
        .transfer_from(alice, Address::ZERO, one)
        .unwrap_err();
        assert!(matches!(err, Error::InvalidReceiver(_)));

        assert_eq!(
            two,
            ({
                contract_vm.set_sender(alice);
                &mut contract
            })
            .allowance(alice, bob)
        );
        assert_eq!(
            two,
            ({
                contract_vm.set_sender(alice);
                &mut contract
            })
            .balance_of(alice)
        );
    }

    #[test]
    fn error_when_transfer_to_invalid_receiver() {
        let contract_vm = TestVM::default();
        let contract_address = Address::from([160u8; 20]);
        contract_vm.set_contract_address(contract_address);
        let mut contract = Erc20::from(&contract_vm);
        let alice = Address::from([1u8; 20]);
        let bob = Address::from([2u8; 20]);

        // Alice approves Bob.
        let one = uint!(1_U256);
        ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .approve(bob, one)
        .unwrap();

        let err = ({
            contract_vm.set_sender(bob);
            &mut contract
        })
        .transfer_from(alice, Address::ZERO, one)
        .unwrap_err();
        assert!(matches!(err, Error::InvalidReceiver(_)));
    }

    #[test]
    fn errors_when_transfer_with_insufficient_allowance() {
        let contract_vm = TestVM::default();
        let contract_address = Address::from([160u8; 20]);
        contract_vm.set_contract_address(contract_address);
        let mut contract = Erc20::from(&contract_vm);
        let alice = Address::from([1u8; 20]);
        let bob = Address::from([2u8; 20]);

        // Mint some tokens for Alice.
        let one = uint!(1_U256);
        ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        ._mint(alice, one)
        .unwrap();
        assert_eq!(
            one,
            ({
                contract_vm.set_sender(alice);
                &mut contract
            })
            .balance_of(alice)
        );

        let err = ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .transfer_from(alice, bob, one)
        .unwrap_err();
        assert!(matches!(err, Error::InsufficientAllowance(_)));
    }

    #[test]
    fn approves_and_reads_allowance() {
        let contract_vm = TestVM::default();
        let contract_address = Address::from([160u8; 20]);
        contract_vm.set_contract_address(contract_address);
        let mut contract = Erc20::from(&contract_vm);
        let alice = Address::from([1u8; 20]);
        let bob = Address::from([2u8; 20]);

        let allowance = ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .allowance(alice, bob);
        assert_eq!(U256::ZERO, allowance);

        let one = uint!(1_U256);
        ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .approve(bob, one)
        .unwrap();
        let allowance = ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .allowance(alice, bob);
        assert_eq!(one, allowance);

        assert_emitted(
            &contract_vm,
            &Approval { owner: alice, spender: bob, value: one },
        );
    }

    #[test]
    fn error_when_approve_for_invalid_spender() {
        let contract_vm = TestVM::default();
        let contract_address = Address::from([160u8; 20]);
        contract_vm.set_contract_address(contract_address);
        let mut contract = Erc20::from(&contract_vm);
        let alice = Address::from([1u8; 20]);

        // alice approves [`Address::ZERO`].
        let one = uint!(1_U256);
        let err = ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .approve(Address::ZERO, one)
        .unwrap_err();
        assert!(matches!(err, Error::InvalidSpender(_)));
    }

    #[test]
    fn error_when_invalid_approver() {
        let contract_vm = TestVM::default();
        let contract_address = Address::from([160u8; 20]);
        contract_vm.set_contract_address(contract_address);
        let mut contract = Erc20::from(&contract_vm);
        let alice = Address::from([1u8; 20]);
        let bob = Address::from([2u8; 20]);

        let one = uint!(1_U256);
        let err = ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        ._approve(Address::ZERO, bob, one, false)
        .unwrap_err();
        assert!(matches!(err, Error::InvalidApprover(_)));
    }

    #[test]
    fn interface_id() {
        let actual = <Erc20 as IErc20>::interface_id();
        let expected: B32 = 0x36372b07_u32.into();
        assert_eq!(actual, expected);
    }

    #[test]
    fn supports_interface() {
        let contract_vm = TestVM::default();
        let contract_address = Address::from([160u8; 20]);
        contract_vm.set_contract_address(contract_address);
        let mut contract = Erc20::from(&contract_vm);
        let alice = Address::from([1u8; 20]);

        assert!(({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .supports_interface(<Erc20 as IErc20>::interface_id()));
        assert!(({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .supports_interface(<Erc20 as IErc165>::interface_id()));

        let fake_interface_id: B32 = 0x12345678_u32.into();
        assert!(!({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .supports_interface(fake_interface_id));
    }
}
