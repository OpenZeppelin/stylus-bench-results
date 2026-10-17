//! Extension of the ERC-20 token contract to support token wrapping.
//!
//! Users can deposit and withdraw "underlying tokens" and receive a matching
//! number of "wrapped tokens". This is useful in conjunction with other
//! modules.
//!
//! WARNING: Any mechanism in which the underlying token changes the
//! [`IErc20::balance_of`] of an account without an explicit transfer may
//! desynchronize this contract's supply and its underlying balance. Please
//! exercise caution when wrapping tokens that may undercollateralize the
//! wrapper (i.e. wrapper's total supply is higher than its underlying balance).
//! See [`Erc20Wrapper::_recover`] for recovering value accrued to the wrapper.

use alloc::{vec, vec::Vec};

use alloy_primitives::{Address, U256, U8};
use openzeppelin_stylus_proc::interface_id;
pub use sol::*;
use stylus_sdk::{
    prelude::Call, prelude::*, storage::StorageAddress,
    stylus_core::calls::errors::MethodError,
};

use crate::token::erc20::{
    self,
    interface::{Erc20Interface, IErc20MetadataInterface},
    utils::{safe_erc20, ISafeErc20, SafeErc20},
    Erc20, IErc20,
};

/// Default number of decimals for an [ERC-20] token.
///
/// [ERC-20]: <https://github.com/OpenZeppelin/openzeppelin-contracts/blob/v5.3.0/contracts/token/ERC20/ERC20.sol>
const DEFAULT_DECIMALS: u8 = 18;

#[cfg_attr(coverage_nightly, coverage(off))]
mod sol {
    use alloy_sol_macro::sol;

    sol! {
        /// Indicates that the address is not a valid ERC-20 token.
        ///
        /// * `token` - Address of the invalid ERC-20 token.
        #[derive(Debug)]
        #[allow(missing_docs)]
        error ERC20InvalidUnderlying(address token);
    }
}

/// An [`Erc20Wrapper`] error.
#[derive(SolidityError, Debug)]
pub enum Error {
    /// Indicates an error related to the current balance of `sender`. Used in
    /// transfers.
    InsufficientBalance(erc20::ERC20InsufficientBalance),
    /// Indicates a failure with the token `sender`. Used in transfers.
    InvalidSender(erc20::ERC20InvalidSender),
    /// Indicates a failure with the token `receiver`. Used in transfers.
    InvalidReceiver(erc20::ERC20InvalidReceiver),
    /// Indicates a failure with the `spender`’s `allowance`. Used in
    /// transfers.
    InsufficientAllowance(erc20::ERC20InsufficientAllowance),
    /// Indicates a failure with the `spender` to be approved. Used in
    /// approvals.
    InvalidSpender(erc20::ERC20InvalidSpender),
    /// Indicates a failure with the `approver` of a token to be approved. Used
    /// in approvals. approver Address initiating an approval operation.
    InvalidApprover(erc20::ERC20InvalidApprover),
    /// An operation with an ERC-20 token failed.
    SafeErc20FailedOperation(safe_erc20::SafeErc20FailedOperation),
    /// Indicates a failed [`ISafeErc20::safe_decrease_allowance`] request.
    SafeErc20FailedDecreaseAllowance(
        safe_erc20::SafeErc20FailedDecreaseAllowance,
    ),
    /// The underlying token couldn't be wrapped.
    InvalidUnderlying(ERC20InvalidUnderlying),
}

impl From<erc20::Error> for Error {
    fn from(value: erc20::Error) -> Self {
        match value {
            erc20::Error::InsufficientBalance(e) => {
                Error::InsufficientBalance(e)
            }
            erc20::Error::InvalidSender(e) => Error::InvalidSender(e),
            erc20::Error::InvalidReceiver(e) => Error::InvalidReceiver(e),
            erc20::Error::InsufficientAllowance(e) => {
                Error::InsufficientAllowance(e)
            }
            erc20::Error::InvalidSpender(e) => Error::InvalidSpender(e),
            erc20::Error::InvalidApprover(e) => Error::InvalidApprover(e),
        }
    }
}

impl From<safe_erc20::Error> for Error {
    fn from(value: safe_erc20::Error) -> Self {
        match value {
            safe_erc20::Error::SafeErc20FailedOperation(e) => {
                Error::SafeErc20FailedOperation(e)
            }
            safe_erc20::Error::SafeErc20FailedDecreaseAllowance(e) => {
                Error::SafeErc20FailedDecreaseAllowance(e)
            }
        }
    }
}

impl MethodError for Error {
    fn encode(self) -> alloc::vec::Vec<u8> {
        self.into()
    }
}

/// State of an [`Erc20Wrapper`] token.
#[storage]
pub struct Erc20Wrapper {
    /// Address of the underlying token.
    pub(crate) underlying: StorageAddress,
    /// [`SafeErc20`] contract.
    safe_erc20: SafeErc20,
}

/// ERC-20 Wrapper Standard Interface
#[interface_id]
#[public]
pub trait IErc20Wrapper {
    /// The error type associated to the trait implementation.
    type Error: Into<alloc::vec::Vec<u8>>;

    /// Returns the number of decimals used to get its user representation.
    ///
    /// # Arguments
    ///
    /// * `&self` - Read access to the contract's state.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// fn decimals(&self) -> U8 {
    ///     self.erc20_wrapper.decimals()
    /// }
    /// ```
    #[must_use]
    fn decimals(&self) -> U8;

    /// Returns the address of the underlying ERC-20 token that is being
    /// wrapped.
    ///
    /// # Arguments
    ///
    /// * `&self` - Read access to the contract's state.
    ///
    /// # Examples
    ///
    /// ```rust,ignore
    /// fn underlying(&self) -> Address {
    ///     self.erc20_wrapper.underlying()
    /// }
    /// ```
    #[must_use]
    fn underlying(&self) -> Address;

    /// Allow a user to deposit underlying tokens and mint the corresponding
    /// number of wrapped tokens.
    ///
    /// # Arguments
    ///
    /// * `&mut self` - Write access to the contract's state.
    /// * `account` - The account to deposit tokens to.
    /// * `value` - The amount of tokens to deposit.
    ///
    /// # Errors
    ///
    /// * [`Error::InvalidSender`] - If the `self.vm().msg_sender()`'s address is the
    ///   `contract:address()`.
    /// * [`Error::InvalidReceiver`] - If the `account` address is a
    ///   `contract:address()`.
    /// * [`Error::SafeErc20FailedOperation`] - If caller lacks sufficient
    ///   balance or hasn't approved enough tokens to the [`Erc20Wrapper`]
    ///   contract.
    /// * [`Error::InvalidReceiver`] - If the `account` address is
    ///   [`Address::ZERO`].
    ///
    /// # Panics
    ///
    /// * If [`Erc20::_mint`] operation panics.
    fn deposit_for(
        &mut self,
        account: Address,
        value: U256,
    ) -> Result<bool, Self::Error>;

    /// Allow a user to burn a number of wrapped tokens and withdraw the
    /// corresponding number of underlying tokens.
    ///
    /// # Arguments
    ///
    /// * `&mut self` - Write access to the contract's state.
    /// * `account` - The account to withdraw tokens from.
    /// * `value` - The amount of tokens to withdraw.
    ///
    /// # Errors
    ///
    /// * [`Error::InvalidReceiver`] - If the `account`'s address is a
    ///   `contract:address()`.
    /// * [`Error::InvalidSender`] - If the `from` address is [`Address::ZERO`].
    /// * [`Error::InsufficientBalance`] - If the `from` address doesn't have
    ///   enough tokens.
    /// * [`Error::SafeErc20FailedOperation`] - If the [`Erc20Wrapper`] contract
    ///   lacks sufficient balance.
    fn withdraw_to(
        &mut self,
        account: Address,
        value: U256,
    ) -> Result<bool, Self::Error>;
}

/// NOTE: Implementation of [`TopLevelStorage`] to be able use `&mut self` when
/// calling other contracts and not `&mut (impl TopLevelStorage +
/// BorrowMut<Self>)`. Should be fixed in the future by the Stylus team.
unsafe impl TopLevelStorage for Erc20Wrapper {}

impl Erc20Wrapper {
    /// See [`IErc20Wrapper::decimals`].
    #[must_use]
    pub fn decimals(&self) -> U8 {
        U8::from(
            IErc20MetadataInterface::new(self.underlying())
                .decimals(self.vm(), Call::new())
                .unwrap_or(DEFAULT_DECIMALS),
        )
    }

    /// See [`IErc20Wrapper::underlying`].
    #[must_use]
    pub fn underlying(&self) -> Address {
        self.underlying.get()
    }

    /// See [`IErc20Wrapper::deposit_for`].
    #[allow(clippy::missing_errors_doc)]
    pub fn deposit_for(
        &mut self,
        account: Address,
        value: U256,
        erc20: &mut Erc20,
    ) -> Result<bool, Error> {
        let contract_address = self.vm().contract_address();
        let sender = self.vm().msg_sender();

        if sender == contract_address {
            return Err(erc20::ERC20InvalidSender { sender }.into());
        }

        if account == contract_address {
            return Err(
                erc20::ERC20InvalidReceiver { receiver: account }.into()
            );
        }

        self.safe_erc20.safe_transfer_from(
            self.underlying(),
            sender,
            contract_address,
            value,
        )?;

        erc20._mint(account, value)?;

        Ok(true)
    }

    /// See [`IErc20Wrapper::withdraw_to`].
    #[allow(clippy::missing_errors_doc)]
    pub fn withdraw_to(
        &mut self,
        account: Address,
        value: U256,
        erc20: &mut Erc20,
    ) -> Result<bool, Error> {
        if account == self.vm().contract_address() {
            return Err(
                erc20::ERC20InvalidReceiver { receiver: account }.into()
            );
        }

        erc20._burn(self.vm().msg_sender(), value)?;

        self.safe_erc20.safe_transfer(self.underlying(), account, value)?;

        Ok(true)
    }
}

#[public]
impl Erc20Wrapper {
    /// Constructor.
    ///
    /// # Arguments
    ///
    /// * `&mut self` - Write access to the contract's state.
    /// * `underlying_token` - The wrapped token.
    ///
    /// # Errors
    ///
    /// * [`Error::InvalidUnderlying`] - If underlying token is this contract.
    #[constructor]
    pub fn constructor(
        &mut self,
        underlying_token: Address,
    ) -> Result<(), Error> {
        if underlying_token == self.vm().contract_address() {
            return Err(Error::InvalidUnderlying(ERC20InvalidUnderlying {
                token: underlying_token,
            }));
        }
        self.underlying.set(underlying_token);
        Ok(())
    }
}

impl Erc20Wrapper {
    /// Mint wrapped token to cover any underlying tokens that would have been
    /// transferred by mistake or acquired from rebasing mechanisms.
    ///
    /// Internal function that can be exposed with access control if desired.
    ///
    /// # Arguments
    ///
    /// * `&mut self` - Write access to the contract's state.
    /// * `account` - The account to mint tokens to.
    /// * `erc20` - Write access to an [`Erc20`] contract.
    ///
    /// # Errors
    ///
    /// * [`Error::InvalidUnderlying`]  - If the external call for
    ///   [`IErc20::balance_of`] fails.
    /// * [`Error::InvalidReceiver`] - If the `account` address is
    ///   [`Address::ZERO`].
    ///
    /// # Panics
    ///
    /// * If the underlying balance is less than the [`IErc20::total_supply`].
    pub fn _recover(
        &mut self,
        account: Address,
        erc20: &mut Erc20,
    ) -> Result<U256, Error> {
        let contract_address = self.vm().contract_address();

        let underlying_token = Erc20Interface::new(self.underlying());

        let underlying_balance = underlying_token
            .balance_of(self.vm(), Call::new(), contract_address)
            .map_err(|_| ERC20InvalidUnderlying { token: self.underlying() })?;

        let value = underlying_balance
            .checked_sub(erc20.total_supply())
            .expect("underlying balance should be greater than the `IErc20::total_supply`");

        if value > U256::ZERO {
            erc20._mint(account, value)?;
        }

        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use alloy_primitives::{aliases::B32, uint};
    use alloy_sol_types::{SolCall, SolError, SolValue};
    use stylus_sdk::{alloy_sol_types::sol, testing::TestVM};

    sol! {
        interface IERC20WrapperTestUnderlying {
            function transferFrom(address from, address to, uint256 value) external returns (bool);
            function transfer(address to, uint256 value) external returns (bool);
            function balanceOf(address account) external view returns (uint256);
            function decimals() external view returns (uint8);
        }
    }

    use super::*;

    const DUMMY_TEST_DECIMALS: u8 = 12;

    fn mock_transfer_from(
        vm: &TestVM,
        token: Address,
        from: Address,
        to: Address,
        value: U256,
        result: Result<Vec<u8>, Vec<u8>>,
    ) {
        vm.set_code(token, vec![1]);
        vm.mock_call(
            token,
            IERC20WrapperTestUnderlying::transferFromCall { from, to, value }
                .abi_encode(),
            U256::ZERO,
            result,
        );
    }

    fn mock_transfer(
        vm: &TestVM,
        token: Address,
        to: Address,
        value: U256,
        result: Result<Vec<u8>, Vec<u8>>,
    ) {
        vm.set_code(token, vec![1]);
        vm.mock_call(
            token,
            IERC20WrapperTestUnderlying::transferCall { to, value }
                .abi_encode(),
            U256::ZERO,
            result,
        );
    }

    fn mock_balance_of(
        vm: &TestVM,
        token: Address,
        account: Address,
        balance: U256,
    ) {
        vm.set_code(token, vec![1]);
        vm.mock_static_call(
            token,
            IERC20WrapperTestUnderlying::balanceOfCall { account }.abi_encode(),
            Ok(balance.abi_encode()),
        );
    }

    #[storage]
    struct Erc20WrapperTestExample {
        wrapper: Erc20Wrapper,
        erc20: Erc20,
    }

    #[public]
    #[implements(IErc20Wrapper<Error = Error>)]
    impl Erc20WrapperTestExample {
        #[constructor]
        fn constructor(
            &mut self,
            underlying_token: Address,
        ) -> Result<(), Error> {
            self.wrapper.constructor(underlying_token)
        }

        fn recover(&mut self, account: Address) -> Result<U256, Error> {
            self.wrapper._recover(account, &mut self.erc20)
        }
    }

    #[public]
    impl IErc20Wrapper for Erc20WrapperTestExample {
        type Error = Error;

        fn decimals(&self) -> U8 {
            self.wrapper.decimals()
        }

        fn underlying(&self) -> Address {
            self.wrapper.underlying()
        }

        fn deposit_for(
            &mut self,
            account: Address,
            value: U256,
        ) -> Result<bool, Error> {
            self.wrapper.deposit_for(account, value, &mut self.erc20)
        }

        fn withdraw_to(
            &mut self,
            account: Address,
            value: U256,
        ) -> Result<bool, Error> {
            self.wrapper.withdraw_to(account, value, &mut self.erc20)
        }
    }

    unsafe impl TopLevelStorage for Erc20WrapperTestExample {}

    use crate::test_utils::assert_emitted;

    #[test]
    fn decimals_works() {
        let vm = TestVM::default();
        let address = Address::from([160u8; 20]);
        let token = Address::from([161u8; 20]);
        let alice = Address::from([1u8; 20]);
        vm.set_contract_address(address);
        let mut contract = Erc20WrapperTestExample::from(&vm);
        ({
            vm.set_sender(alice);
            &mut contract
        })
        .constructor(token)
        .expect("construct");
        vm.set_code(token, vec![1]);
        vm.mock_static_call(
            token,
            IERC20WrapperTestUnderlying::decimalsCall {}.abi_encode(),
            Ok(U256::from(DUMMY_TEST_DECIMALS).abi_encode()),
        );
        assert_eq!(
            ({
                vm.set_sender(alice);
                &mut contract
            })
            .decimals(),
            U8::from(DUMMY_TEST_DECIMALS)
        );
    }

    #[test]
    fn underlying_works() {
        let vm = TestVM::default();
        vm.set_contract_address(Address::from([160u8; 20]));
        let mut contract = Erc20WrapperTestExample::from(&vm);
        let token = Address::from([161u8; 20]);
        ({
            vm.set_sender(Address::from([1u8; 20]));
            &mut contract
        })
        .constructor(token)
        .expect("construct");
        assert_eq!(contract.wrapper.underlying(), token);
    }

    #[test]
    fn constructor_reverts_when_invalid_asset() {
        let contract_vm = TestVM::default();
        let contract_address = Address::from([160u8; 20]);
        contract_vm.set_contract_address(contract_address);
        let mut contract = Erc20WrapperTestExample::from(&contract_vm);
        let alice = Address::from([1u8; 20]);

        let invalid_asset = contract_address;

        let err = ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .constructor(invalid_asset)
        .expect_err("should return Error::InvalidUnderlying");

        assert!(matches!(
            err,
            Error::InvalidUnderlying(ERC20InvalidUnderlying { token })
                if token == invalid_asset
        ));
    }

    #[test]
    fn deposit_for_reverts_when_invalid_asset() {
        let contract_vm = TestVM::default();
        let contract_address = Address::from([160u8; 20]);
        contract_vm.set_contract_address(contract_address);
        let mut contract = Erc20WrapperTestExample::from(&contract_vm);
        let alice = Address::from([1u8; 20]);

        // assume an invalid underlying asset is somehow set in the contract
        let invalid_asset = alice;
        ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .wrapper
        .underlying
        .set(invalid_asset);

        let err = ({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .deposit_for(alice, uint!(10_U256))
        .expect_err("should return Error::SafeErc20FailedOperation");

        assert!(matches!(
            err,
            Error::SafeErc20FailedOperation(
                safe_erc20::SafeErc20FailedOperation { token }
            ) if token == invalid_asset
        ));
    }

    #[test]
    fn deposit_for_reverts_when_invalid_sender() {
        let vm = TestVM::default();
        let address = Address::from([160u8; 20]);
        vm.set_contract_address(address);
        let mut contract = Erc20WrapperTestExample::from(&vm);
        let alice = Address::from([1u8; 20]);
        ({
            vm.set_sender(alice);
            &mut contract
        })
        .constructor(Address::from([161u8; 20]))
        .expect("construct");
        let err = ({
            vm.set_sender(address);
            &mut contract
        })
        .deposit_for(alice, uint!(10_U256))
        .unwrap_err();
        assert!(
            matches!(err, Error::InvalidSender(erc20::ERC20InvalidSender { sender }) if sender == address)
        );
    }

    #[test]
    fn deposit_for_reverts_when_invalid_receiver() {
        let vm = TestVM::default();
        let address = Address::from([160u8; 20]);
        vm.set_contract_address(address);
        let mut contract = Erc20WrapperTestExample::from(&vm);
        ({
            vm.set_sender(Address::from([1u8; 20]));
            &mut contract
        })
        .constructor(Address::from([161u8; 20]))
        .expect("construct");
        let err = ({
            vm.set_sender(Address::from([1u8; 20]));
            &mut contract
        })
        .deposit_for(address, uint!(10_U256))
        .unwrap_err();
        assert!(
            matches!(err, Error::InvalidReceiver(erc20::ERC20InvalidReceiver { receiver }) if receiver == address)
        );
    }

    #[test]
    fn deposit_for_reverts_when_insufficient_allowance() {
        let vm = TestVM::default();
        let address = Address::from([160u8; 20]);
        let token = Address::from([161u8; 20]);
        let alice = Address::from([1u8; 20]);
        let amount = uint!(10_U256);
        vm.set_contract_address(address);
        let mut contract = Erc20WrapperTestExample::from(&vm);
        ({
            vm.set_sender(alice);
            &mut contract
        })
        .constructor(token)
        .expect("construct");
        mock_transfer_from(
            &vm,
            token,
            alice,
            address,
            amount,
            Err(erc20::ERC20InsufficientAllowance {
                spender: address,
                allowance: U256::ZERO,
                needed: amount,
            }
            .abi_encode()),
        );
        let err = ({
            vm.set_sender(alice);
            &mut contract
        })
        .deposit_for(alice, amount)
        .unwrap_err();
        assert!(
            matches!(err, Error::SafeErc20FailedOperation(safe_erc20::SafeErc20FailedOperation { token: failed }) if failed == token)
        );
    }

    #[test]
    fn deposit_for_reverts_when_insufficient_balance() {
        let vm = TestVM::default();
        let address = Address::from([160u8; 20]);
        let token = Address::from([161u8; 20]);
        let alice = Address::from([1u8; 20]);
        let amount = uint!(10_U256);
        let needed = amount + uint!(1_U256);
        vm.set_contract_address(address);
        let mut contract = Erc20WrapperTestExample::from(&vm);
        ({
            vm.set_sender(alice);
            &mut contract
        })
        .constructor(token)
        .expect("construct");
        mock_transfer_from(
            &vm,
            token,
            alice,
            address,
            needed,
            Err(erc20::ERC20InsufficientBalance {
                sender: alice,
                balance: amount,
                needed,
            }
            .abi_encode()),
        );
        let err = ({
            vm.set_sender(alice);
            &mut contract
        })
        .deposit_for(alice, needed)
        .unwrap_err();
        assert!(
            matches!(err, Error::SafeErc20FailedOperation(safe_erc20::SafeErc20FailedOperation { token: failed }) if failed == token)
        );
    }

    #[test]
    fn deposit_for_works() {
        let vm = TestVM::default();
        let address = Address::from([160u8; 20]);
        let token = Address::from([161u8; 20]);
        let alice = Address::from([1u8; 20]);
        let amount = uint!(10_U256);
        vm.set_contract_address(address);
        let mut contract = Erc20WrapperTestExample::from(&vm);
        ({
            vm.set_sender(alice);
            &mut contract
        })
        .constructor(token)
        .expect("construct");
        mock_transfer_from(&vm, token, alice, address, amount, Ok(Vec::new()));
        let balance = ({
            vm.set_sender(alice);
            &mut contract
        })
        .erc20
        .balance_of(alice);
        let supply = ({
            vm.set_sender(alice);
            &mut contract
        })
        .erc20
        .total_supply();
        assert!(({
            vm.set_sender(alice);
            &mut contract
        })
        .deposit_for(alice, amount)
        .expect("deposit"));
        assert_emitted(
            &vm,
            &erc20::Transfer { from: Address::ZERO, to: alice, value: amount },
        );
        assert_eq!(
            ({
                vm.set_sender(alice);
                &mut contract
            })
            .erc20
            .balance_of(alice),
            balance + amount
        );
        assert_eq!(
            ({
                vm.set_sender(alice);
                &mut contract
            })
            .erc20
            .total_supply(),
            supply + amount
        );
    }

    #[test]
    fn withdraw_to_reverts_when_invalid_receiver() {
        let vm = TestVM::default();
        let address = Address::from([160u8; 20]);
        let alice = Address::from([1u8; 20]);
        vm.set_contract_address(address);
        let mut contract = Erc20WrapperTestExample::from(&vm);
        ({
            vm.set_sender(alice);
            &mut contract
        })
        .constructor(Address::from([161u8; 20]))
        .expect("construct");
        let err = ({
            vm.set_sender(alice);
            &mut contract
        })
        .withdraw_to(address, uint!(10_U256))
        .unwrap_err();
        assert!(
            matches!(err, Error::InvalidReceiver(erc20::ERC20InvalidReceiver { receiver }) if receiver == address)
        );
    }

    #[test]
    fn withdraw_to_reverts_when_insufficient_balance() {
        let vm = TestVM::default();
        let address = Address::from([160u8; 20]);
        let token = Address::from([161u8; 20]);
        let alice = Address::from([1u8; 20]);
        let amount = uint!(10_U256);
        let needed = amount + uint!(1_U256);
        vm.set_contract_address(address);
        let mut contract = Erc20WrapperTestExample::from(&vm);
        ({
            vm.set_sender(alice);
            &mut contract
        })
        .constructor(token)
        .expect("construct");
        mock_transfer_from(&vm, token, alice, address, amount, Ok(Vec::new()));
        ({
            vm.set_sender(alice);
            &mut contract
        })
        .deposit_for(alice, amount)
        .expect("deposit");
        let err = ({
            vm.set_sender(alice);
            &mut contract
        })
        .withdraw_to(alice, needed)
        .unwrap_err();
        assert!(
            matches!(err, Error::InsufficientBalance(erc20::ERC20InsufficientBalance { sender, balance, needed: requested }) if sender == alice && balance == amount && requested == needed)
        );
    }

    #[test]
    fn withdraw_to_works() {
        let vm = TestVM::default();
        let address = Address::from([160u8; 20]);
        let token = Address::from([161u8; 20]);
        let alice = Address::from([1u8; 20]);
        let amount = uint!(10_U256);
        vm.set_contract_address(address);
        let mut contract = Erc20WrapperTestExample::from(&vm);
        ({
            vm.set_sender(alice);
            &mut contract
        })
        .constructor(token)
        .expect("construct");
        mock_transfer_from(&vm, token, alice, address, amount, Ok(Vec::new()));
        ({
            vm.set_sender(alice);
            &mut contract
        })
        .deposit_for(alice, amount)
        .expect("deposit");
        let balance = ({
            vm.set_sender(alice);
            &mut contract
        })
        .erc20
        .balance_of(alice);
        let supply = ({
            vm.set_sender(alice);
            &mut contract
        })
        .erc20
        .total_supply();
        mock_transfer(&vm, token, alice, amount, Ok(Vec::new()));
        assert!(({
            vm.set_sender(alice);
            &mut contract
        })
        .withdraw_to(alice, amount)
        .expect("withdraw"));
        assert_emitted(
            &vm,
            &erc20::Transfer { from: alice, to: Address::ZERO, value: amount },
        );
        assert_eq!(
            ({
                vm.set_sender(alice);
                &mut contract
            })
            .erc20
            .balance_of(alice),
            balance - amount
        );
        assert_eq!(
            ({
                vm.set_sender(alice);
                &mut contract
            })
            .erc20
            .total_supply(),
            supply - amount
        );
    }

    #[test]
    fn recover_reverts_when_invalid_underlying() {
        let vm = TestVM::default();
        let address = Address::from([160u8; 20]);
        let invalid = Address::from([1u8; 20]);
        vm.set_contract_address(address);
        let mut contract = Erc20WrapperTestExample::from(&vm);
        ({
            vm.set_sender(invalid);
            &mut contract
        })
        .wrapper
        .underlying
        .set(invalid);
        vm.set_code(invalid, vec![1]);
        vm.mock_static_call(
            invalid,
            IERC20WrapperTestUnderlying::balanceOfCall { account: address }
                .abi_encode(),
            Err(Vec::new()),
        );
        let err = ({
            vm.set_sender(invalid);
            &mut contract
        })
        .recover(invalid)
        .unwrap_err();
        assert!(
            matches!(err, Error::InvalidUnderlying(ERC20InvalidUnderlying { token }) if token == invalid)
        );
        assert_eq!(
            ({
                vm.set_sender(invalid);
                &mut contract
            })
            .erc20
            .balance_of(invalid),
            U256::ZERO
        );
        assert_eq!(
            ({
                vm.set_sender(invalid);
                &mut contract
            })
            .erc20
            .total_supply(),
            U256::ZERO
        );
    }

    #[test]
    #[should_panic = "underlying balance should be greater than the `IErc20::total_supply`"]
    fn recover_panics_when_underlying_balance_is_less_than_total_supply() {
        let vm = TestVM::default();
        let address = Address::from([160u8; 20]);
        let token = Address::from([161u8; 20]);
        let alice = Address::from([1u8; 20]);
        let amount = uint!(10_U256);
        vm.set_contract_address(address);
        let mut contract = Erc20WrapperTestExample::from(&vm);
        ({
            vm.set_sender(alice);
            &mut contract
        })
        .constructor(token)
        .expect("construct");
        mock_transfer_from(&vm, token, alice, address, amount, Ok(Vec::new()));
        ({
            vm.set_sender(alice);
            &mut contract
        })
        .deposit_for(alice, amount)
        .expect("deposit");
        ({
            vm.set_sender(alice);
            &mut contract
        })
        .erc20
        ._mint(alice, amount)
        .expect("mint");
        mock_balance_of(&vm, token, address, amount);
        _ = ({
            vm.set_sender(alice);
            &mut contract
        })
        .recover(alice);
    }

    #[test]
    fn recover_works_when_underlying_balance_is_equal_to_total_supply() {
        let vm = TestVM::default();
        let address = Address::from([160u8; 20]);
        let token = Address::from([161u8; 20]);
        let alice = Address::from([1u8; 20]);
        let amount = uint!(10_U256);
        vm.set_contract_address(address);
        let mut contract = Erc20WrapperTestExample::from(&vm);
        ({
            vm.set_sender(alice);
            &mut contract
        })
        .constructor(token)
        .expect("construct");
        mock_transfer_from(&vm, token, alice, address, amount, Ok(Vec::new()));
        ({
            vm.set_sender(alice);
            &mut contract
        })
        .deposit_for(alice, amount)
        .expect("deposit");
        mock_balance_of(&vm, token, address, amount);
        assert_eq!(
            ({
                vm.set_sender(alice);
                &mut contract
            })
            .recover(alice)
            .expect("recover"),
            U256::ZERO
        );
    }

    #[test]
    fn recover_works_when_underlying_balance_is_greater_than_total_supply() {
        let vm = TestVM::default();
        let address = Address::from([160u8; 20]);
        let token = Address::from([161u8; 20]);
        let alice = Address::from([1u8; 20]);
        let amount = uint!(10_U256);
        let delta = uint!(1_U256);
        vm.set_contract_address(address);
        let mut contract = Erc20WrapperTestExample::from(&vm);
        ({
            vm.set_sender(alice);
            &mut contract
        })
        .constructor(token)
        .expect("construct");
        mock_transfer_from(&vm, token, alice, address, amount, Ok(Vec::new()));
        ({
            vm.set_sender(alice);
            &mut contract
        })
        .deposit_for(alice, amount)
        .expect("deposit");
        mock_balance_of(&vm, token, address, amount + delta);
        assert_eq!(
            ({
                vm.set_sender(alice);
                &mut contract
            })
            .recover(alice)
            .expect("recover"),
            delta
        );
        assert_emitted(
            &vm,
            &erc20::Transfer { from: Address::ZERO, to: alice, value: delta },
        );
    }

    #[test]
    fn interface_id() {
        let actual = <Erc20WrapperTestExample as IErc20Wrapper>::interface_id();
        let expected: B32 = 0x511f913e_u32.into();
        assert_eq!(actual, expected);
    }
}
