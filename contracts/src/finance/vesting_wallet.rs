//! A vesting wallet handles the vesting of Ether and ERC-20 tokens for a given
//! beneficiary.
//!
//! A vesting wallet is an ownable contract that can receive native currency and
//! [`crate::token::erc20::Erc20`] tokens, and release these assets to the
//! wallet owner, also referred to as "beneficiary", according to a vesting
//! schedule.
//!
//! Any assets transferred to this contract will follow the vesting schedule as
//! if they were locked from the beginning. Consequently, if the vesting has
//! already started, any amount of tokens sent to this contract will (at least
//! partly) be immediately releasable.
//!
//! By setting the duration to 0, one can configure this contract to behave like
//! an asset timelock that hold tokens for a beneficiary until a specified time.
//!
//! NOTE: Since the wallet is [`Ownable`], and ownership
//! can be transferred, it is possible to sell unvested tokens. Preventing this
//! in a smart contract is difficult, considering that: 1) a beneficiary address
//! could be a counterfactually deployed contract, 2) there is likely to be a
//! migration path for EOAs to become contracts in the near future.
//!
//! NOTE: When using this contract with any token whose balance is adjusted
//! automatically (i.e. a rebase token), make sure to account the supply/balance
//! adjustment in the vesting schedule to ensure the vested amount is as
//! intended.

use alloc::{
    string::{String, ToString},
    vec,
    vec::Vec,
};

use alloy_primitives::{aliases::B32, Address, U256, U64};
use openzeppelin_stylus_proc::interface_id;
pub use sol::*;
use stylus_sdk::{
    function_selector,
    prelude::*,
    storage::{StorageMap, StorageU256, StorageU64},
    stylus_core::calls::errors::{Error as CallError, MethodError},
};

use crate::{
    access::ownable::{self, Ownable},
    token::erc20::{
        interface::Erc20Interface,
        utils::{safe_erc20, ISafeErc20, SafeErc20},
    },
    utils::{introspection::erc165::IErc165, math::storage::AddAssignChecked},
};

#[cfg_attr(coverage_nightly, coverage(off))]
mod sol {
    use alloy_sol_macro::sol;

    sol! {
        /// Emitted when `amount` of Ether has been released.
        ///
        /// * `amount` - Total Ether released.
        #[derive(Debug)]
        #[allow(missing_docs)]
        event EtherReleased(uint256 amount);

        /// Emitted when `amount` of ERC-20 `token` has been released.
        ///
        /// * `token` - Address of the token being released.
        /// * `amount` - Number of tokens released.
        #[derive(Debug)]
        #[allow(missing_docs)]
        event ERC20Released(address indexed token, uint256 amount);
    }

    sol! {
        /// Indicates an error related to the underlying Ether transfer.
        #[derive(Debug)]
        #[allow(missing_docs)]
        error ReleaseEtherFailed(string reason);

        /// Indicates that a low-level call failed.
        #[derive(Debug)]
        #[allow(missing_docs)]
        error FailedCall();

        /// The token address is not valid (eg. [`Address::ZERO`]).
        ///
        /// * `token` - Address of the token being released.
        #[derive(Debug)]
        #[allow(missing_docs)]
        error InvalidToken(address token);
    }
}

/// An error that occurred in the [`VestingWallet`] contract.
#[derive(SolidityError, Debug)]
pub enum Error {
    /// The caller account is not authorized to perform an operation.
    UnauthorizedAccount(ownable::OwnableUnauthorizedAccount),
    /// The owner is not a valid owner account. (eg. [`Address::ZERO`]).
    InvalidOwner(ownable::OwnableInvalidOwner),
    /// Indicates an error related to the underlying Ether transfer.
    ReleaseEtherFailed(ReleaseEtherFailed),
    /// Indicates that a low-level call failed.
    FailedCall(FailedCall),
    /// An operation with an ERC-20 token failed.
    SafeErc20FailedOperation(safe_erc20::SafeErc20FailedOperation),
    /// Indicates a failed [`ISafeErc20::safe_decrease_allowance`] request.
    SafeErc20FailedDecreaseAllowance(
        safe_erc20::SafeErc20FailedDecreaseAllowance,
    ),
    /// The token address is not valid. (eg. [`Address::ZERO`]).
    InvalidToken(InvalidToken),
}

impl From<ownable::Error> for Error {
    fn from(value: ownable::Error) -> Self {
        match value {
            ownable::Error::UnauthorizedAccount(e) => {
                Error::UnauthorizedAccount(e)
            }
            ownable::Error::InvalidOwner(e) => Error::InvalidOwner(e),
        }
    }
}

impl From<CallError> for Error {
    fn from(value: CallError) -> Self {
        match value {
            CallError::AbiDecodingFailed(_) => Error::FailedCall(FailedCall {}),
            CallError::Revert(reason) => {
                Error::ReleaseEtherFailed(ReleaseEtherFailed {
                    reason: String::from_utf8_lossy(&reason).to_string(),
                })
            }
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

/// State of a [`VestingWallet`] Contract.
#[storage]
pub struct VestingWallet {
    /// [`Ownable`] contract.
    // We leave the parent [`Ownable`] contract instance public, so that
    // inheritting contract have access to its internal functions.
    pub ownable: Ownable,
    /// Amount of Ether already released.
    pub(crate) released: StorageU256,
    /// Amount of ERC-20 tokens already released.
    pub(crate) erc20_released: StorageMap<Address, StorageU256>,
    /// Start timestamp.
    pub(crate) start: StorageU64,
    /// Vesting duration.
    pub(crate) duration: StorageU64,
    /// [`SafeErc20`] contract.
    safe_erc20: SafeErc20,
}

/// NOTE: Implementation of [`TopLevelStorage`] to be able use `&mut self` when
/// calling other contracts and not `&mut (impl TopLevelStorage +
/// BorrowMut<Self>)`. Should be fixed in the future by the Stylus team.
unsafe impl TopLevelStorage for VestingWallet {}

/// Required interface of a [`VestingWallet`] compliant contract.
#[interface_id]
#[public]
// The `#[public]` macro adds an undocumented helper method to the trait.
#[allow(missing_docs)]
pub trait IVestingWallet {
    /// The error type associated to this trait implementation.
    type Error: Into<alloc::vec::Vec<u8>>;

    /// Returns the address of the current owner.
    ///
    /// Re-export of [`Ownable::owner`].
    ///
    /// # Arguments
    ///
    /// * `&self` - Read access to the contract's state.
    fn owner(&self) -> Address;

    /// Transfers ownership of the contract to a new account (`new_owner`). Can
    /// only be called by the current owner.
    ///
    /// Re-export of [`Ownable::transfer_ownership`].
    ///
    /// # Arguments
    ///
    /// * `&mut self` - Write access to the contract's state.
    /// * `new_owner` - The next owner of this contract.
    ///
    /// # Errors
    ///
    /// * [`ownable::Error::UnauthorizedAccount`] - If called by any account
    ///   other than the owner.
    /// * [`ownable::Error::InvalidOwner`] - If `new_owner` is the
    ///   [`Address::ZERO`].
    ///
    /// # Events
    ///
    /// * [`ownable::OwnershipTransferred`].
    fn transfer_ownership(
        &mut self,
        new_owner: Address,
    ) -> Result<(), Self::Error>;

    /// Leaves the contract without owner. It will not be possible to call
    /// [`Ownable::only_owner`] functions. Can only be called by the current
    /// owner.
    ///
    /// Re-export of [`Ownable::renounce_ownership`].
    ///
    /// NOTE: Renouncing ownership will leave the contract without an owner,
    /// thereby disabling any functionality that is only available to the owner.
    ///
    /// # Arguments
    ///
    /// * `&mut self` - Write access to the contract's state.
    ///
    /// # Errors
    ///
    /// * [`ownable::Error::UnauthorizedAccount`] - If not called by the owner.
    ///
    /// # Events
    ///
    /// * [`ownable::OwnershipTransferred`].
    fn renounce_ownership(&mut self) -> Result<(), Self::Error>;

    /// Getter for the start timestamp.
    ///
    /// # Arguments
    ///
    /// * `&self` - Read access to the contract's state.
    fn start(&self) -> U256;

    /// Getter for the vesting duration.
    ///
    /// # Arguments
    ///
    /// * `&self` - Read access to the contract's state.
    fn duration(&self) -> U256;

    /// Getter for the end timestamp.
    ///
    /// # Arguments
    ///
    /// * `&self` - Read access to the contract's state.
    fn end(&self) -> U256;

    /// Amount of Ether already released.
    ///
    /// # Arguments
    ///
    /// * `&self` - Read access to the contract's state.
    #[selector(name = "released")]
    fn released_eth(&self) -> U256;

    /// Amount of token already released.
    ///
    /// # Arguments
    ///
    /// * `&self` - Read access to the contract's state.
    /// * `token` - Address of the token being released.
    #[selector(name = "released")]
    fn released_erc20(&self, token: Address) -> U256;

    /// Getter for the amount of releasable Ether.
    ///
    /// # Arguments
    ///
    /// * `&self` - Read access to the contract's state.
    ///
    /// # Panics
    ///
    /// * If total allocation exceeds [`U256::MAX`].
    /// * If scaled, total allocation (mid calculation) exceeds [`U256::MAX`].
    #[selector(name = "releasable")]
    fn releasable_eth(&self) -> U256;

    /// Getter for the amount of releasable `token` tokens. `token` should be
    /// the address of an ERC-20 contract.
    ///
    /// # Arguments
    ///
    /// * `&mut self` - Write access to the contract's state.
    /// * `token` - Address of the releasable token.
    ///
    /// # Errors
    ///
    /// * [`Error::InvalidToken`] - If the `token` address is not a contract.
    ///
    /// # Panics
    ///
    /// * If total allocation exceeds [`U256::MAX`].
    /// * If scaled, total allocation (mid calculation) exceeds [`U256::MAX`].
    #[selector(name = "releasable")]
    fn releasable_erc20(&mut self, token: Address)
        -> Result<U256, Self::Error>;

    /// Release the native tokens (Ether) that have already vested.
    ///
    /// # Arguments
    ///
    /// * `&mut self` - Write access to the contract's state.
    ///
    /// # Errors
    ///
    /// * [`Error::ReleaseEtherFailed`] - If Ether transfer fails.
    ///
    /// # Events
    ///
    /// * [`EtherReleased`].
    ///
    /// # Panics
    ///
    /// * If total allocation exceeds [`U256::MAX`].
    /// * If scaled total allocation (mid calculation) exceeds [`U256::MAX`].
    #[selector(name = "release")]
    fn release_eth(&mut self) -> Result<(), Self::Error>;

    /// Release the tokens that have already vested.
    ///
    /// # Arguments
    ///
    /// * `&mut self` - Write access to the contract's state.
    /// * `token` - Address of the token being released.
    ///
    /// # Errors
    ///
    /// * [`Error::InvalidToken`] -  If the `token` address is not a contract.
    /// * [`safe_erc20::Error::SafeErc20FailedOperation`] - If the contract
    ///   fails to execute the call.
    ///
    /// # Events
    ///
    /// * [`ERC20Released`].
    ///
    /// # Panics
    ///
    /// * If total allocation exceeds [`U256::MAX`].
    /// * If scaled, total allocation (mid calculation) exceeds [`U256::MAX`].
    #[selector(name = "release")]
    fn release_erc20(&mut self, token: Address) -> Result<(), Self::Error>;

    /// Calculates the amount of Ether that has already vested.
    /// The Default implementation is a linear vesting curve.
    ///
    /// # Arguments
    ///
    /// * `&self` - Read access to the contract's state.
    /// * `timestamp` - Point in time for which to check the vested amount.
    ///
    /// # Panics
    ///
    /// * If total allocation exceeds [`U256::MAX`].
    /// * If scaled, total allocation (mid calculation) exceeds [`U256::MAX`].
    #[selector(name = "vestedAmount")]
    fn vested_amount_eth(&self, timestamp: u64) -> U256;

    /// Calculates the amount of tokens that has already vested.
    /// The Default implementation is a linear vesting curve.
    ///
    /// # Arguments
    ///
    /// * `&mut self` - Write access to the contract's state.
    /// * `token` - Address of the token being released.
    /// * `timestamp` - Point in time for which to check the vested amount.
    ///
    /// # Errors
    ///
    /// * [`Error::InvalidToken`] - If the `token` address is not a contract.
    ///
    /// # Panics
    ///
    /// * If total allocation exceeds [`U256::MAX`].
    /// * If scaled, total allocation (mid calculation) exceeds [`U256::MAX`].
    #[selector(name = "vestedAmount")]
    fn vested_amount_erc20(
        &mut self,
        token: Address,
        timestamp: u64,
    ) -> Result<U256, Self::Error>;
}

#[public]
#[implements(IVestingWallet<Error = Error>, IErc165)]
impl VestingWallet {
    /// Constructor.
    ///
    /// # Arguments
    ///
    /// * `&mut self` - Write access to the contract's state.
    /// * `beneficiary` - The wallet owner.
    /// * `start_timestamp` - The point in time when token vesting starts.
    /// * `duration_seconds` - The vesting duration in seconds.
    ///
    /// # Errors
    ///
    /// * [`ownable::Error::InvalidOwner`] - If beneficiary is
    ///   [`Address::ZERO`].
    #[constructor]
    pub fn constructor(
        &mut self,
        beneficiary: Address,
        start_timestamp: U64,
        duration_seconds: U64,
    ) -> Result<(), Error> {
        self.ownable.constructor(beneficiary)?;
        self.start.set(start_timestamp);
        self.duration.set(duration_seconds);
        Ok(())
    }

    /// The contract should be able to receive Eth.
    ///
    /// # Errors
    ///
    /// * If the transaction includes data (non-zero calldata).
    /// * If the contract doesn't have enough gas to execute the function.
    #[receive]
    pub fn receive(&mut self) -> Result<(), Vec<u8>> {
        Ok(())
    }
}

#[public]
impl IVestingWallet for VestingWallet {
    type Error = Error;

    fn owner(&self) -> Address {
        self.ownable.owner()
    }

    fn transfer_ownership(
        &mut self,
        new_owner: Address,
    ) -> Result<(), Self::Error> {
        Ok(self.ownable.transfer_ownership(new_owner)?)
    }

    fn renounce_ownership(&mut self) -> Result<(), Self::Error> {
        Ok(self.ownable.renounce_ownership()?)
    }

    fn start(&self) -> U256 {
        U256::from(self.start.get())
    }

    fn duration(&self) -> U256 {
        U256::from(self.duration.get())
    }

    fn end(&self) -> U256 {
        // SAFETY: both `start` and `duration` are stored as [`U64`], so they
        // cannot exceed [`U256::MAX`].
        self.start() + self.duration()
    }

    #[selector(name = "released")]
    fn released_eth(&self) -> U256 {
        self.released.get()
    }

    #[selector(name = "released")]
    fn released_erc20(&self, token: Address) -> U256 {
        self.erc20_released.get(token)
    }

    #[selector(name = "releasable")]
    fn releasable_eth(&self) -> U256 {
        // SAFETY: total vested amount is by definition greater than or equal to
        // the released amount.
        self.vested_amount_eth(self.vm().block_timestamp())
            - self.released_eth()
    }

    #[selector(name = "releasable")]
    fn releasable_erc20(
        &mut self,
        token: Address,
    ) -> Result<U256, Self::Error> {
        let vested =
            self.vested_amount_erc20(token, self.vm().block_timestamp())?;
        // SAFETY: total vested amount is by definition greater than or equal to
        // the released amount.
        Ok(vested - self.released_erc20(token))
    }

    #[selector(name = "release")]
    fn release_eth(&mut self) -> Result<(), Self::Error> {
        let amount = self.releasable_eth();

        self.released.add_assign_checked(
            amount,
            "total released should not exceed `U256::MAX`",
        );

        let owner = self.ownable.owner();

        let context = Call::new_payable(self, amount);
        call(self.vm(), context, owner, &[])?;

        self.vm().log(EtherReleased { amount });

        Ok(())
    }

    #[selector(name = "release")]
    fn release_erc20(&mut self, token: Address) -> Result<(), Self::Error> {
        let amount = self.releasable_erc20(token)?;
        let owner = self.ownable.owner();

        self.erc20_released.setter(token).add_assign_checked(
            amount,
            "total released should not exceed `U256::MAX`",
        );

        self.safe_erc20.safe_transfer(token, owner, amount)?;

        self.vm().log(ERC20Released { token, amount });

        Ok(())
    }

    #[selector(name = "vestedAmount")]
    fn vested_amount_eth(&self, timestamp: u64) -> U256 {
        let total_allocation = self
            .vm()
            .balance(self.vm().contract_address())
            .checked_add(self.released_eth())
            .expect("total allocation should not exceed `U256::MAX`");

        self.vesting_schedule(total_allocation, U64::from(timestamp))
    }

    #[selector(name = "vestedAmount")]
    fn vested_amount_erc20(
        &mut self,
        token: Address,
        timestamp: u64,
    ) -> Result<U256, Self::Error> {
        let erc20 = Erc20Interface::new(token);
        let balance = erc20
            .balance_of(self.vm(), Call::new(), self.vm().contract_address())
            .map_err(|_| InvalidToken { token })?;

        let total_allocation = balance
            .checked_add(self.released_erc20(token))
            .expect("total allocation should not exceed `U256::MAX`");

        Ok(self.vesting_schedule(total_allocation, U64::from(timestamp)))
    }
}

impl VestingWallet {
    /// Virtual implementation of the vesting formula. This returns the amount
    /// vested, as a function of time, for an asset given its total
    /// historical allocation.
    ///
    /// # Arguments
    ///
    /// * `&self` - Read access to the contract's state.
    /// * `total_allocation` - Total vested amount.
    /// * `timestamp` - Point in time for which to calculate the vested amount.
    ///
    /// # Panics
    ///
    /// * If scaled, total allocation (mid calculation) exceeds [`U256::MAX`].
    fn vesting_schedule(&self, total_allocation: U256, timestamp: U64) -> U256 {
        let timestamp = U256::from(timestamp);

        if timestamp < self.start() {
            U256::ZERO
        } else if timestamp >= self.end() {
            total_allocation
        } else {
            // SAFETY: `timestamp` is guaranteed to be greater than
            // `self.start()` as checked by earlier bounds.
            let elapsed = timestamp - self.start();

            let scaled_allocation = total_allocation
                .checked_mul(elapsed)
                .expect("scaled allocation exceeds `U256::MAX`");

            // SAFETY: `self.duration()` is non-zero. If `self.duration()` were
            // zero, then `end == start`, meaning that `timestamp >= self.end()`
            // and the function would have returned earlier.
            scaled_allocation / self.duration()
        }
    }
}

#[public]
impl IErc165 for VestingWallet {
    fn supports_interface(&self, interface_id: B32) -> bool {
        <Self as IVestingWallet>::interface_id() == interface_id
            || self.ownable.supports_interface(interface_id)
            || <Self as IErc165>::interface_id() == interface_id
    }
}

#[cfg(test)]
mod tests {
    use alloy_primitives::{address, uint};
    use alloy_sol_types::{sol, SolCall};
    use stylus_sdk::testing::TestVM;

    use super::*;

    sol! {
        function balanceOf(address account) external view returns (uint256);
    }

    const BALANCE: u64 = 1000;

    const DURATION: u64 = 4 * 365 * 86400; // 4 years

    /// Block timestamp of the tests: 1st January 2025, 00:00:00 UTC.
    const TIMESTAMP: u64 = 1_735_689_600;

    /// Address of the mocked ERC-20 token contract.
    const ERC20: Address =
        address!("0x0000000000000000000000000000000000000e20");

    fn start(vm: &TestVM) -> u64 {
        vm.block_timestamp() + 3600 // 1 hour
    }

    /// Mocks the response of the ERC-20 `token` to a `balanceOf(account)`
    /// call.
    fn mock_balance_of(
        vm: &TestVM,
        token: Address,
        account: Address,
        balance: U256,
    ) {
        let calldata = balanceOfCall { account }.abi_encode();
        let returns = balanceOfCall::abi_encode_returns(&balance);
        vm.mock_static_call(token, calldata, Ok(returns));
    }

    impl VestingWallet {
        fn init(&mut self, start: u64, duration: u64) -> (U64, U64) {
            let start = U64::from(start);
            let duration = U64::from(duration);
            self.start.set(start);
            self.duration.set(duration);
            (start, duration)
        }
    }

    #[test]
    fn reads_start() {
        let vm = TestVM::default();
        vm.set_block_timestamp(TIMESTAMP);
        let mut contract = VestingWallet::from(&vm);

        let (start, _) = contract.init(start(&vm), DURATION);
        assert_eq!(U256::from(start), contract.start());
    }

    #[test]
    fn reads_duration() {
        let vm = TestVM::default();
        let mut contract = VestingWallet::from(&vm);

        let (_, duration) = contract.init(0, DURATION);
        assert_eq!(U256::from(duration), contract.duration());
    }

    #[test]
    fn reads_end() {
        let vm = TestVM::default();
        vm.set_block_timestamp(TIMESTAMP);
        let mut contract = VestingWallet::from(&vm);

        let (start, duration) = contract.init(start(&vm), DURATION);

        assert_eq!(U256::from(start + duration), contract.end());
    }

    #[test]
    fn reads_max_end() {
        let vm = TestVM::default();
        let mut contract = VestingWallet::from(&vm);

        contract.init(u64::MAX, u64::MAX);
        assert_eq!(U256::from(U64::MAX) + U256::from(U64::MAX), contract.end());
    }

    #[test]
    fn gets_vesting_schedule() {
        let vm = TestVM::default();
        vm.set_block_timestamp(TIMESTAMP);
        let mut contract = VestingWallet::from(&vm);

        let (start, duration) = contract.init(start(&vm), DURATION);

        let one = uint!(1_U256);
        let two = uint!(2_U256);

        assert_eq!(
            U256::ZERO,
            contract.vesting_schedule(two, start - U64::from(1))
        );
        assert_eq!(
            one,
            contract.vesting_schedule(two, start + duration / U64::from(2))
        );
        assert_eq!(two, contract.vesting_schedule(two, start + duration));
        assert_eq!(
            two,
            contract.vesting_schedule(two, start + duration + U64::from(1))
        );
    }

    #[test]
    fn gets_vesting_schedule_zero_duration() {
        let vm = TestVM::default();
        vm.set_block_timestamp(TIMESTAMP);
        let mut contract = VestingWallet::from(&vm);

        let (start, _) = contract.init(start(&vm), 0);

        let two = uint!(2_U256);

        assert_eq!(
            U256::ZERO,
            contract.vesting_schedule(two, start - U64::from(1))
        );
        assert_eq!(two, contract.vesting_schedule(two, start));
        assert_eq!(two, contract.vesting_schedule(two, start + U64::from(1)));
    }

    #[test]
    fn check_vested_amount_erc20() {
        let vm = TestVM::default();
        vm.set_block_timestamp(TIMESTAMP);
        let mut contract = VestingWallet::from(&vm);

        contract.init(start(&vm), DURATION);
        mock_balance_of(&vm, ERC20, vm.contract_address(), U256::from(BALANCE));

        let start = start(&vm);
        for i in 0..64 {
            let timestamp = i * DURATION / 60 + start;
            let expected_amount = U256::from(std::cmp::min(
                BALANCE,
                BALANCE * (timestamp - start) / DURATION,
            ));

            let vested_amount =
                contract.vested_amount_erc20(ERC20, timestamp).unwrap();
            assert_eq!(
                expected_amount, vested_amount,
                "\n---\ni: {i}\nstart: {start}\ntimestamp: {timestamp}\n---\n"
            );
        }
    }

    #[test]
    fn interface_id() {
        let actual = <VestingWallet as IVestingWallet>::interface_id();
        let expected: B32 = 0x23a2649d_u32.into();
        assert_ne!(actual, expected);
    }

    #[test]
    fn supports_interface() {
        let vm = TestVM::default();
        let contract = VestingWallet::from(&vm);

        assert!(contract.supports_interface(
            <VestingWallet as IVestingWallet>::interface_id()
        ));
        assert!(contract
            .supports_interface(<VestingWallet as IErc165>::interface_id()));

        let fake_interface_id: B32 = 0x12345678_u32.into();
        assert!(!contract.supports_interface(fake_interface_id));
    }
}
