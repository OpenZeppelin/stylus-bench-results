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
    prelude::*, storage::StorageAddress,
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
// The `#[public]` macro adds an undocumented helper method to the trait.
#[allow(missing_docs)]
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
    /// * [`Error::InvalidSender`] - If the caller's address is the address of
    ///   this contract.
    /// * [`Error::InvalidReceiver`] - If the `account` address is the address
    ///   of this contract.
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
    /// * [`Error::InvalidReceiver`] - If the `account`'s address is the address
    ///   of this contract.
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
            .map_err(|_| ERC20InvalidUnderlying { token: contract_address })?;

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
    use alloy_primitives::{address, aliases::B32, uint};
    use alloy_sol_types::{sol, SolCall, SolValue};
    use stylus_sdk::testing::TestVM;

    use super::*;
    use crate::{test_utils::ALICE, token::erc20::tests::assert_emitted};

    sol! {
        /// The calls that the wrapper makes to its underlying token.
        interface IERC20 {
            function balanceOf(address account) external view returns (uint256);
            function decimals() external view returns (uint8);
            function transfer(address to, uint256 value) external returns (bool);
            function transferFrom(address from, address to, uint256 value) external returns (bool);
        }
    }

    const DUMMY_TEST_DECIMALS: u8 = 12;

    /// Address of the underlying token of the wrapper.
    const UNDERLYING: Address =
        address!("0x00000000000000000000000000000000000e2c20");

    /// The ERC-20 token wrapped by the contract under test.
    ///
    /// A [`TestVM`] cannot call into another contract, so the token lives in
    /// its own VM and the calls that the wrapper makes to it are mocked with
    /// the outcome of the same calls on the token. The `TestVM` returns the
    /// data of the last registered mock to every call, so a mock is registered
    /// right before the call that reads it.
    struct Underlying {
        vm: TestVM,
        token: Erc20,
    }

    impl Underlying {
        /// Deploys the token at [`UNDERLYING`], where `caller` sees a contract.
        fn deploy(caller: &TestVM) -> Self {
            let vm = TestVM::default();
            vm.set_contract_address(UNDERLYING);
            let token = Erc20::from(&vm);

            caller.set_code(UNDERLYING, vec![1]);

            Self { vm, token }
        }

        /// Mocks the `transferFrom` call that `caller` makes to the token.
        fn mock_transfer_from(
            &mut self,
            caller: &TestVM,
            from: Address,
            to: Address,
            value: U256,
        ) {
            self.vm.set_sender(caller.contract_address());
            let result = self.token.transfer_from(from, to, value);
            caller.mock_call(
                UNDERLYING,
                IERC20::transferFromCall { from, to, value }.abi_encode(),
                U256::ZERO,
                Self::response(result),
            );
        }

        /// Mocks the `transfer` call that `caller` makes to the token.
        fn mock_transfer(&mut self, caller: &TestVM, to: Address, value: U256) {
            self.vm.set_sender(caller.contract_address());
            let result = self.token.transfer(to, value);
            caller.mock_call(
                UNDERLYING,
                IERC20::transferCall { to, value }.abi_encode(),
                U256::ZERO,
                Self::response(result),
            );
        }

        /// Mocks the `balanceOf` call that `caller` makes to the token.
        fn mock_balance_of(&self, caller: &TestVM, account: Address) {
            caller.mock_static_call(
                UNDERLYING,
                IERC20::balanceOfCall { account }.abi_encode(),
                Self::response(Ok(self.token.balance_of(account))),
            );
        }

        /// Encodes the outcome of a call on the token as the response to the
        /// call.
        fn response<T: SolValue>(
            result: Result<T, erc20::Error>,
        ) -> Result<Vec<u8>, Vec<u8>> {
            result.map(|value| value.abi_encode()).map_err(Into::into)
        }
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

    #[test]
    fn decimals_works() {
        let vm = TestVM::default();
        let mut contract = Erc20WrapperTestExample::from(&vm);

        vm.mock_static_call(
            UNDERLYING,
            IERC20::decimalsCall {}.abi_encode(),
            Ok(IERC20::decimalsCall::abi_encode_returns(&DUMMY_TEST_DECIMALS)),
        );

        contract.constructor(UNDERLYING).expect("should construct");
        assert_eq!(contract.decimals(), U8::from(DUMMY_TEST_DECIMALS));
    }

    #[test]
    fn underlying_works() {
        let vm = TestVM::default();
        let mut contract = Erc20WrapperTestExample::from(&vm);
        let underlying = Underlying::deploy(&vm);

        let erc20_address = underlying.vm.contract_address();

        contract.constructor(erc20_address).expect("should construct");

        assert_eq!(contract.underlying(), erc20_address);
    }

    #[test]
    fn constructor_reverts_when_invalid_asset() {
        let vm = TestVM::default();
        let mut contract = Erc20WrapperTestExample::from(&vm);

        let invalid_asset = vm.contract_address();

        let err = contract
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
        let vm = TestVM::default();
        let mut contract = Erc20WrapperTestExample::from(&vm);

        // assume an invalid underlying asset is somehow set in the contract
        let invalid_asset = ALICE;
        contract.wrapper.underlying.set(invalid_asset);

        vm.set_sender(ALICE);
        let err = contract
            .deposit_for(ALICE, uint!(10_U256))
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
        let mut contract = Erc20WrapperTestExample::from(&vm);
        let underlying = Underlying::deploy(&vm);

        let invalid_sender = vm.contract_address();

        contract
            .constructor(underlying.vm.contract_address())
            .expect("should construct");

        vm.set_sender(invalid_sender);
        let err = contract
            .deposit_for(ALICE, uint!(10_U256))
            .expect_err("should return Error::InvalidSender");

        assert!(matches!(
            err,
            Error::InvalidSender(erc20::ERC20InvalidSender { sender }) if sender == invalid_sender
        ));
    }

    #[test]
    fn deposit_for_reverts_when_invalid_receiver() {
        let vm = TestVM::default();
        let mut contract = Erc20WrapperTestExample::from(&vm);
        let underlying = Underlying::deploy(&vm);

        let invalid_receiver = vm.contract_address();

        contract
            .constructor(underlying.vm.contract_address())
            .expect("should construct");

        vm.set_sender(ALICE);
        let err = contract
            .deposit_for(invalid_receiver, uint!(10_U256))
            .expect_err("should return Error::InvalidReceiver");

        assert!(matches!(
            err,
            Error::InvalidReceiver(erc20::ERC20InvalidReceiver { receiver }) if receiver == invalid_receiver
        ));
    }

    #[test]
    fn deposit_for_reverts_when_insufficient_allowance() {
        let vm = TestVM::default();
        let mut contract = Erc20WrapperTestExample::from(&vm);
        let mut underlying = Underlying::deploy(&vm);

        let amount = uint!(10_U256);

        contract
            .constructor(underlying.vm.contract_address())
            .expect("should construct");

        underlying.token._mint(ALICE, amount).expect("should mint");

        underlying.mock_transfer_from(
            &vm,
            ALICE,
            vm.contract_address(),
            amount,
        );

        vm.set_sender(ALICE);
        let err = contract
            .deposit_for(ALICE, amount)
            .expect_err("should return Error::SafeErc20");

        assert!(matches!(
            err,
            Error::SafeErc20FailedOperation(
                safe_erc20::SafeErc20FailedOperation { token }
            ) if token == underlying.vm.contract_address()
        ));
    }

    #[test]
    fn deposit_for_reverts_when_insufficient_balance() {
        let vm = TestVM::default();
        let mut contract = Erc20WrapperTestExample::from(&vm);
        let mut underlying = Underlying::deploy(&vm);

        let amount = uint!(10_U256);

        let exceeding_value = amount + uint!(1_U256);

        contract
            .constructor(underlying.vm.contract_address())
            .expect("should construct");

        underlying.token._mint(ALICE, amount).expect("should mint");

        underlying.vm.set_sender(ALICE);
        underlying
            .token
            .approve(vm.contract_address(), exceeding_value)
            .expect("should approve");

        underlying.mock_transfer_from(
            &vm,
            ALICE,
            vm.contract_address(),
            exceeding_value,
        );

        vm.set_sender(ALICE);
        let err = contract
            .deposit_for(ALICE, exceeding_value)
            .expect_err("should return Error::SafeErc20");

        assert!(matches!(
            err,
            Error::SafeErc20FailedOperation(
                safe_erc20::SafeErc20FailedOperation { token }
            ) if token == underlying.vm.contract_address()
        ));
    }

    #[test]
    fn deposit_for_works() {
        let vm = TestVM::default();
        let mut contract = Erc20WrapperTestExample::from(&vm);
        let mut underlying = Underlying::deploy(&vm);

        let amount = uint!(10_U256);

        contract
            .constructor(underlying.vm.contract_address())
            .expect("should construct");

        underlying.token._mint(ALICE, amount).expect("should mint");

        let initial_balance = underlying.token.balance_of(ALICE);
        let initial_wrapped_balance = contract.erc20.balance_of(ALICE);

        let initial_contract_balance =
            underlying.token.balance_of(vm.contract_address());

        let initial_wrapped_supply = contract.erc20.total_supply();

        underlying.vm.set_sender(ALICE);
        underlying
            .token
            .approve(vm.contract_address(), amount)
            .expect("should approve");

        underlying.mock_transfer_from(
            &vm,
            ALICE,
            vm.contract_address(),
            amount,
        );

        vm.set_sender(ALICE);
        assert!(contract.deposit_for(ALICE, amount).expect("should deposit"));

        assert_emitted(
            &underlying.vm,
            &erc20::Transfer {
                from: ALICE,
                to: vm.contract_address(),
                value: amount,
            },
        );

        assert_emitted(
            &vm,
            &erc20::Transfer { from: Address::ZERO, to: ALICE, value: amount },
        );

        assert_eq!(
            underlying.token.balance_of(ALICE),
            initial_balance - amount
        );

        assert_eq!(
            contract.erc20.balance_of(ALICE),
            initial_wrapped_balance + amount
        );

        assert_eq!(
            underlying.token.balance_of(vm.contract_address()),
            initial_contract_balance + amount
        );

        assert_eq!(
            contract.erc20.total_supply(),
            initial_wrapped_supply + amount
        );
    }

    #[test]
    fn withdraw_to_reverts_when_invalid_receiver() {
        let vm = TestVM::default();
        let mut contract = Erc20WrapperTestExample::from(&vm);
        let underlying = Underlying::deploy(&vm);

        let invalid_receiver = vm.contract_address();
        contract
            .constructor(underlying.vm.contract_address())
            .expect("should construct");

        vm.set_sender(ALICE);
        let err = contract
            .withdraw_to(invalid_receiver, uint!(10_U256))
            .expect_err("should return Error::InvalidReceiver");

        assert!(matches!(
            err,
            Error::InvalidReceiver(erc20::ERC20InvalidReceiver { receiver }) if receiver == invalid_receiver
        ));
    }

    #[test]
    fn withdraw_to_reverts_when_insufficient_balance() {
        let vm = TestVM::default();
        let mut contract = Erc20WrapperTestExample::from(&vm);
        let mut underlying = Underlying::deploy(&vm);

        let amount = uint!(10_U256);

        contract
            .constructor(underlying.vm.contract_address())
            .expect("should construct");

        underlying.token._mint(ALICE, amount).expect("should mint");

        underlying.vm.set_sender(ALICE);
        underlying
            .token
            .approve(vm.contract_address(), amount)
            .expect("should approve");

        underlying.mock_transfer_from(
            &vm,
            ALICE,
            vm.contract_address(),
            amount,
        );

        vm.set_sender(ALICE);
        contract.deposit_for(ALICE, amount).expect("should deposit");

        let exceeding_value = amount + uint!(1_U256);

        let err = contract
            .withdraw_to(ALICE, exceeding_value)
            .expect_err("should return Error::SafeErc20");

        assert!(matches!(
            err,
            Error::InsufficientBalance(
                erc20::ERC20InsufficientBalance {
                    sender,
                    balance,
                    needed
                }
            ) if sender == ALICE && balance == amount && needed == exceeding_value
        ));
    }

    #[test]
    fn withdraw_to_works() {
        let vm = TestVM::default();
        let mut contract = Erc20WrapperTestExample::from(&vm);
        let mut underlying = Underlying::deploy(&vm);

        let amount = uint!(10_U256);

        contract
            .constructor(underlying.vm.contract_address())
            .expect("should construct");

        underlying.token._mint(ALICE, amount).expect("should mint");

        underlying.vm.set_sender(ALICE);
        underlying
            .token
            .approve(vm.contract_address(), amount)
            .expect("should approve");

        underlying.mock_transfer_from(
            &vm,
            ALICE,
            vm.contract_address(),
            amount,
        );

        vm.set_sender(ALICE);
        contract.deposit_for(ALICE, amount).expect("should deposit");

        let initial_balance = underlying.token.balance_of(ALICE);
        let initial_wrapped_balance = contract.erc20.balance_of(ALICE);

        let initial_contract_balance =
            underlying.token.balance_of(vm.contract_address());

        let initial_wrapped_supply = contract.erc20.total_supply();

        underlying.mock_transfer(&vm, ALICE, amount);

        assert!(contract.withdraw_to(ALICE, amount).expect("should withdraw"));

        assert_emitted(
            &vm,
            &erc20::Transfer { from: ALICE, to: Address::ZERO, value: amount },
        );

        assert_emitted(
            &underlying.vm,
            &erc20::Transfer {
                from: vm.contract_address(),
                to: ALICE,
                value: amount,
            },
        );

        assert_eq!(
            underlying.token.balance_of(ALICE),
            initial_balance + amount
        );

        assert_eq!(
            contract.erc20.balance_of(ALICE),
            initial_wrapped_balance - amount
        );

        assert_eq!(
            underlying.token.balance_of(vm.contract_address()),
            initial_contract_balance - amount
        );

        assert_eq!(
            contract.erc20.total_supply(),
            initial_wrapped_supply - amount
        );
    }

    #[test]
    #[ignore = "`_recover` reports the address of the wrapper, not the one of the underlying token, in `ERC20InvalidUnderlying`"]
    fn recover_reverts_when_invalid_underlying() {
        let vm = TestVM::default();
        let mut contract = Erc20WrapperTestExample::from(&vm);

        let invalid_underlying = ALICE;
        contract.wrapper.underlying.set(invalid_underlying);

        vm.set_sender(ALICE);
        let err = contract
            .recover(ALICE)
            .expect_err("should return Error::InvalidUnderlying");

        assert!(matches!(
            err, Error::InvalidUnderlying(ERC20InvalidUnderlying { token }) if token == invalid_underlying
        ));
    }

    #[test]
    #[should_panic = "underlying balance should be greater than the `IErc20::total_supply`"]
    fn recover_panics_when_underlying_balance_is_less_than_total_supply() {
        let vm = TestVM::default();
        let mut contract = Erc20WrapperTestExample::from(&vm);
        let mut underlying = Underlying::deploy(&vm);

        let amount = uint!(10_U256);

        contract
            .constructor(underlying.vm.contract_address())
            .expect("should construct");

        underlying.token._mint(ALICE, amount).expect("should mint");

        underlying.vm.set_sender(ALICE);
        underlying
            .token
            .approve(vm.contract_address(), amount)
            .expect("should approve");

        underlying.mock_transfer_from(
            &vm,
            ALICE,
            vm.contract_address(),
            amount,
        );

        vm.set_sender(ALICE);
        contract.deposit_for(ALICE, amount).expect("should deposit");

        // Unexpected mint.
        contract.erc20._mint(ALICE, amount).expect("should mint");

        underlying.mock_balance_of(&vm, vm.contract_address());

        // This should panic.
        _ = contract.recover(ALICE);
    }

    #[test]
    fn recover_works_when_underlying_balance_is_equal_to_total_supply() {
        let vm = TestVM::default();
        let mut contract = Erc20WrapperTestExample::from(&vm);
        let mut underlying = Underlying::deploy(&vm);

        let amount = uint!(10_U256);

        contract
            .constructor(underlying.vm.contract_address())
            .expect("should construct");

        underlying.token._mint(ALICE, amount).expect("should mint");

        underlying.vm.set_sender(ALICE);
        underlying
            .token
            .approve(vm.contract_address(), amount)
            .expect("should approve");

        underlying.mock_transfer_from(
            &vm,
            ALICE,
            vm.contract_address(),
            amount,
        );

        vm.set_sender(ALICE);
        contract.deposit_for(ALICE, amount).expect("should deposit");

        underlying.mock_balance_of(&vm, vm.contract_address());

        assert_eq!(
            contract.recover(ALICE).expect("should recover"),
            U256::ZERO
        );
    }

    #[test]
    fn recover_works_when_underlying_balance_is_greater_than_total_supply() {
        let vm = TestVM::default();
        let mut contract = Erc20WrapperTestExample::from(&vm);
        let mut underlying = Underlying::deploy(&vm);

        let amount = uint!(10_U256);

        contract
            .constructor(underlying.vm.contract_address())
            .expect("should construct");

        underlying.token._mint(ALICE, amount).expect("should mint");

        underlying.vm.set_sender(ALICE);
        underlying
            .token
            .approve(vm.contract_address(), amount)
            .expect("should approve");

        underlying.mock_transfer_from(
            &vm,
            ALICE,
            vm.contract_address(),
            amount,
        );

        vm.set_sender(ALICE);
        contract.deposit_for(ALICE, amount).expect("should deposit");

        // Unexpected mint.
        let unexpected_delta = uint!(1_U256);
        underlying
            .token
            ._mint(vm.contract_address(), unexpected_delta)
            .expect("should mint");

        underlying.mock_balance_of(&vm, vm.contract_address());

        assert_eq!(
            contract.recover(ALICE).expect("should recover"),
            unexpected_delta
        );

        assert_emitted(
            &vm,
            &erc20::Transfer {
                from: Address::ZERO,
                to: ALICE,
                value: unexpected_delta,
            },
        );
    }

    #[test]
    fn interface_id() {
        let actual = <Erc20WrapperTestExample as IErc20Wrapper>::interface_id();
        let expected: B32 = 0x511f913e_u32.into();
        assert_eq!(actual, expected);
    }
}
