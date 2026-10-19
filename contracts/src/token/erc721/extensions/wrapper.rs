//! Extension of the ERC-721 token contract to support token wrapping.
//!
//! Users can deposit and withdraw an "underlying token" and receive a "wrapped
//! token" with a matching token ID. This is useful in conjunction with other
//! modules.
use alloc::{
    string::{String, ToString},
    vec,
    vec::Vec,
};

use alloy_primitives::{aliases::B32, Address, U256};
use openzeppelin_stylus_proc::interface_id;
pub use sol::*;
use stylus_sdk::{
    abi::Bytes,
    prelude::*,
    storage::StorageAddress,
    stylus_core::calls::errors::{Error as CallError, MethodError},
};

use crate::token::erc721::{
    self, interface::Erc721Interface, receiver::IErc721Receiver, Erc721,
    RECEIVER_FN_SELECTOR,
};

#[cfg_attr(coverage_nightly, coverage(off))]
mod sol {
    use alloy_sol_macro::sol;

    sol! {
        /// The received ERC-721 token couldn't be wrapped.
        ///
        /// * `token` - The token address.
        #[derive(Debug)]
        #[allow(missing_docs)]
        error ERC721UnsupportedToken(address token);

        /// An operation with an ERC-721 token failed.
        ///
        /// * `token` - Address of the ERC-721 token.
        #[derive(Debug)]
        #[allow(missing_docs)]
        error Erc721FailedOperation(address token);

    }
}

/// An [`Erc721Wrapper`] error.
#[derive(SolidityError, Debug)]
pub enum Error {
    /// Indicates that an address can't be an owner.
    /// For example, [`Address::ZERO`] is a forbidden owner in [`Erc721`].
    /// Used in balance queries.
    InvalidOwner(erc721::ERC721InvalidOwner),
    /// Indicates a `token_id` whose `owner` is the zero address.
    NonexistentToken(erc721::ERC721NonexistentToken),
    /// Indicates an error related to the ownership over a particular token.
    /// Used in transfers.
    IncorrectOwner(erc721::ERC721IncorrectOwner),
    /// Indicates a failure with the token `sender`. Used in transfers.
    InvalidSender(erc721::ERC721InvalidSender),
    /// Indicates a failure with the token `receiver`. Used in transfers.
    InvalidReceiver(erc721::ERC721InvalidReceiver),
    /// Indicates a failure with the token `receiver`, with the reason
    /// specified by it.
    InvalidReceiverWithReason(erc721::InvalidReceiverWithReason),
    /// Indicates a failure with the `operator`’s approval. Used in transfers.
    InsufficientApproval(erc721::ERC721InsufficientApproval),
    /// Indicates a failure with the `approver` of a token to be approved. Used
    /// in approvals.
    InvalidApprover(erc721::ERC721InvalidApprover),
    /// Indicates a failure with the `operator` to be approved. Used in
    /// approvals.
    InvalidOperator(erc721::ERC721InvalidOperator),
    /// The received ERC-721 token couldn't be wrapped.
    UnsupportedToken(ERC721UnsupportedToken),
    /// An operation with an ERC-721 token failed.
    Erc721FailedOperation(Erc721FailedOperation),
}

impl From<erc721::Error> for Error {
    fn from(value: erc721::Error) -> Self {
        match value {
            erc721::Error::InvalidOwner(e) => Error::InvalidOwner(e),
            erc721::Error::NonexistentToken(e) => Error::NonexistentToken(e),
            erc721::Error::IncorrectOwner(e) => Error::IncorrectOwner(e),
            erc721::Error::InvalidSender(e) => Error::InvalidSender(e),
            erc721::Error::InvalidReceiver(e) => Error::InvalidReceiver(e),
            erc721::Error::InvalidReceiverWithReason(e) => {
                Error::InvalidReceiverWithReason(e)
            }
            erc721::Error::InsufficientApproval(e) => {
                Error::InsufficientApproval(e)
            }
            erc721::Error::InvalidApprover(e) => Error::InvalidApprover(e),
            erc721::Error::InvalidOperator(e) => Error::InvalidOperator(e),
        }
    }
}

impl MethodError for Error {
    fn encode(self) -> alloc::vec::Vec<u8> {
        self.into()
    }
}

/// State of an [`Erc721Wrapper`] token.
#[storage]
pub struct Erc721Wrapper {
    /// Address of the underlying token.
    underlying: StorageAddress,
}

/// NOTE: Implementation of [`TopLevelStorage`] to be able use `&mut self` when
/// calling other contracts and not `&mut (impl TopLevelStorage +
/// BorrowMut<Self>)`. Should be fixed in the future by the Stylus team.
unsafe impl TopLevelStorage for Erc721Wrapper {}

/// Interface of an extension of the ERC-721 token contract that supports token
/// wrapping.
#[interface_id]
#[public]
// The `#[public]` macro adds an undocumented helper method to the trait.
#[allow(missing_docs)]
pub trait IErc721Wrapper: IErc721Receiver {
    /// The error type associated to this trait implementation.
    type Error: Into<alloc::vec::Vec<u8>>;

    /// Allow a user to deposit underlying tokens and mint the corresponding
    /// `token_ids`.
    ///
    /// # Arguments
    ///
    /// * `&mut self` - Write access to the contract's state.
    /// * `account` - The account to deposit tokens to.
    /// * `token_ids` - List of underlying token ids to deposit.
    ///
    /// # Errors
    ///
    /// * [`Error::Erc721FailedOperation`] - If the underlying token is not an
    ///   ERC-721 contract.
    /// * [`Error::InvalidReceiverWithReason`] - If an error occurs during
    ///   [`erc721::IErc721::transfer_from`] operation on the underlying token.
    /// * [`Error::InvalidSender`] - If `token_id` already exists.
    /// * [`Error::InvalidReceiver`] - If `to` is [`Address::ZERO`].
    /// * [`Error::InvalidReceiver`] - If
    ///   [`erc721::IErc721Receiver::on_erc721_received`] hasn't returned its
    ///   interface id or returned with an error.
    fn deposit_for(
        &mut self,
        account: Address,
        token_ids: Vec<U256>,
    ) -> Result<bool, Self::Error>;

    /// Allow a user to burn wrapped tokens and withdraw the corresponding
    /// `token_ids` of the underlying tokens.
    ///
    /// # Arguments
    ///
    /// * `&mut self` - Write access to the contract's state.
    /// * `account` - The account to withdraw tokens to.
    /// * `token_ids` - List of underlying token ids to withdraw.
    ///
    /// # Errors
    ///
    /// * [`Error::Erc721FailedOperation`] - If the underlying token is not an
    ///   ERC-721 contract.
    /// * [`Error::InvalidReceiverWithReason`] - If an error occurs during
    ///   [`erc721::IErc721::safe_transfer_from`] operation on the underlying
    ///   token.
    /// * [`Error::NonexistentToken`] - If the token does not exist and `auth`
    ///   is not [`Address::ZERO`].
    /// * [`Error::InsufficientApproval`] - If `auth` is not [`Address::ZERO`]
    ///   and `auth` does not have a right to approve this token.
    fn withdraw_to(
        &mut self,
        account: Address,
        token_ids: Vec<U256>,
    ) -> Result<bool, Self::Error>;

    /// Returns the underlying token.
    ///
    /// # Arguments
    ///
    /// * `&self` - Read access to the contract's state.
    #[must_use]
    fn underlying(&self) -> Address;
}

impl Erc721Wrapper {
    /// Constructor.
    ///
    /// # Arguments
    ///
    /// * `&mut self` - Write access to the contract's state.
    /// * `underlying_token` - The wrapped token.
    pub fn constructor(&mut self, underlying_token: Address) {
        self.underlying.set(underlying_token);
    }

    /// Check [`IErc721Wrapper::deposit_for()`] for more information.
    #[allow(clippy::missing_errors_doc)]
    pub fn deposit_for(
        &mut self,
        account: Address,
        token_ids: Vec<U256>,
        erc721: &mut Erc721,
    ) -> Result<bool, Error> {
        let sender = self.vm().msg_sender();
        let contract_address = self.vm().contract_address();
        let underlying = Erc721Interface::new(self.underlying());

        for token_id in token_ids {
            // This is an "unsafe" transfer that doesn't call any hook on
            // the receiver. With [`IErc721Wrapper::underlying()`] being trusted
            // (by design of this contract) and no other contracts expected to
            // be called from there, we are safe.
            let call = Call::new_mutating(self);
            match underlying.transfer_from(
                self.vm(),
                call,
                sender,
                contract_address,
                token_id,
            ) {
                Ok(()) => (),
                Err(e) => {
                    if let CallError::Revert(ref reason) = e {
                        if !reason.is_empty() {
                            return Err(Error::InvalidReceiverWithReason(
                                erc721::InvalidReceiverWithReason {
                                    reason: String::from_utf8_lossy(reason)
                                        .to_string(),
                                },
                            ));
                        }
                    }
                    return Err(Error::Erc721FailedOperation(
                        Erc721FailedOperation { token: self.underlying() },
                    ));
                }
            }

            erc721._safe_mint(account, token_id, &vec![].into())?;
        }

        Ok(true)
    }

    /// Check [`IErc721Wrapper::withdraw_to()`] for more information.
    #[allow(clippy::missing_errors_doc)]
    pub fn withdraw_to(
        &mut self,
        account: Address,
        token_ids: Vec<U256>,
        erc721: &mut Erc721,
    ) -> Result<bool, Error> {
        let sender = self.vm().msg_sender();
        let underlying = Erc721Interface::new(self.underlying());

        for token_id in token_ids {
            // Setting the `auth` argument enables the `_is_authorized` check
            // which verifies that the token exists (from != 0).
            // Therefore, it is not needed to verify that the return value is
            // not 0 here.
            erc721._update(Address::ZERO, token_id, sender)?;
            let call = Call::new_mutating(self);
            match underlying.safe_transfer_from(
                self.vm(),
                call,
                self.vm().contract_address(),
                account,
                token_id,
                vec![].into(),
            ) {
                Ok(()) => (),
                Err(e) => {
                    if let CallError::Revert(ref reason) = e {
                        if !reason.is_empty() {
                            return Err(Error::InvalidReceiverWithReason(
                                erc721::InvalidReceiverWithReason {
                                    reason: String::from_utf8_lossy(reason)
                                        .to_string(),
                                },
                            ));
                        }
                    }
                    return Err(Error::Erc721FailedOperation(
                        Erc721FailedOperation { token: self.underlying() },
                    ));
                }
            }
        }

        Ok(true)
    }

    /// Allow minting on direct ERC-721 transfers to this contract.
    ///
    /// # Arguments
    ///
    /// * `&mut self` - Write access to the contract's state.
    /// * `operator` - The operator of the transfer.
    /// * `from` - The sender of the transfer.
    /// * `token_id` - The token id of the transfer.
    /// * `data` - The data of the transfer.
    /// * `erc721` - Write access to an [`Erc721`] contract.
    ///
    /// # Errors
    ///
    /// * [`Error::UnsupportedToken`] - If the message sender is not the
    ///   underlying token.
    /// * [`Error::InvalidSender`] - If `token_id` already exists.
    /// * [`Error::InvalidReceiver`] - If `to` is [`Address::ZERO`].
    /// * [`Error::InvalidReceiver`] - If
    ///   [`erc721::IErc721Receiver::on_erc721_received`] hasn't returned its
    ///   interface id or returned with an error.
    pub fn on_erc721_received(
        &mut self,
        _operator: Address,
        from: Address,
        token_id: U256,
        _data: &Bytes,
        erc721: &mut Erc721,
    ) -> Result<B32, Error> {
        let sender = self.vm().msg_sender();
        if self.underlying() != sender {
            return Err(Error::UnsupportedToken(ERC721UnsupportedToken {
                token: sender,
            }));
        }

        erc721._safe_mint(from, token_id, &vec![].into())?;

        Ok(RECEIVER_FN_SELECTOR)
    }

    /// Check [`IErc721Wrapper::underlying()`] for more information.
    #[must_use]
    pub fn underlying(&self) -> Address {
        self.underlying.get()
    }

    /// Mints wrapped tokens to cover any underlying tokens that would have been
    /// function that can be exposed with access control if desired.
    ///
    /// # Arguments
    ///
    /// * `&mut self` - Write access to the contract's state.
    /// * `account` - The account to mint tokens to.
    /// * `token_id` - A mutable reference to the Erc20 contract.
    /// * `erc721` - Write access to an [`Erc721`] contract.
    ///
    /// # Errors
    ///
    /// * [`Error::Erc721FailedOperation`] - If the underlying token is not a
    ///   [`Erc721`] contract, or the contract fails to execute the call.
    /// * [`Error::IncorrectOwner`] - If the underlying token is not owned by
    ///   the contract.
    /// * [`Error::InvalidSender`] - If `token_id` already exists.
    /// * [`Error::InvalidReceiver`] - If `to` is [`Address::ZERO`].
    /// * [`Error::InvalidReceiver`] - If
    ///   [`erc721::IErc721Receiver::on_erc721_received`] hasn't returned its
    ///   interface id or returned with an error.
    fn _recover(
        &mut self,
        account: Address,
        token_id: U256,
        erc721: &mut Erc721,
    ) -> Result<U256, Error> {
        let underlying = Erc721Interface::new(self.underlying());

        let owner = underlying
            .owner_of(self.vm(), Call::new(), token_id)
            .map_err(|_| {
                Error::Erc721FailedOperation(Erc721FailedOperation {
                    token: self.underlying(),
                })
            })?;

        let contract_address = self.vm().contract_address();
        if owner != contract_address {
            return Err(erc721::Error::IncorrectOwner(
                erc721::ERC721IncorrectOwner {
                    sender: contract_address,
                    token_id,
                    owner,
                },
            )
            .into());
        }

        erc721._safe_mint(account, token_id, &vec![].into())?;

        Ok(token_id)
    }
}

#[cfg(test)]
mod tests {
    use alloy_primitives::{address, uint};
    use alloy_sol_macro::sol;
    use alloy_sol_types::{SolCall, SolError, SolEvent};
    use stylus_sdk::{abi::Bytes, testing::TestVM};

    use super::*;
    use crate::{
        test_utils::{ALICE, BOB},
        token::erc721::{self, IErc721},
        utils::introspection::erc165::IErc165,
    };

    /// Address of the underlying token of the wrappers under test.
    const UNDERLYING: Address =
        address!("0x000000000000000000000000000000000000c0de");

    sol! {
        // The calls a wrapper makes to its underlying token.
        function transferFrom(address from, address to, uint256 token_id) external;
        function safeTransferFrom(address from, address to, uint256 token_id, bytes data) external;
        function ownerOf(uint256 token_id) external view returns (address owner);
    }

    pub(crate) fn random_token_ids(size: usize) -> Vec<U256> {
        (0..size).map(U256::from).collect()
    }

    /// Asserts that `event` is among the logs emitted in `vm`.
    fn assert_emitted(vm: &TestVM, event: &impl SolEvent) {
        let log = event.encode_log_data();
        let emitted = (log.topics().to_vec(), log.data.to_vec());
        assert!(
            vm.get_emitted_logs().contains(&emitted),
            "event should be emitted"
        );
    }

    /// The underlying [`Erc721`] token of a wrapper.
    ///
    /// A [`TestVM`] doesn't run other contracts: the calls a wrapper makes to
    /// its underlying token are answered with mocks. The answers come from a
    /// real [`Erc721`], deployed at [`UNDERLYING`] in a VM of its own, which
    /// runs the call on behalf of the wrapper when the mock is set. That way
    /// the returned values and the revert data are the token's own, and the
    /// state and the events of the token can be checked like those of any
    /// other contract.
    ///
    /// NOTE: A [`TestVM`] doesn't record calls, and a call that matches no mock
    /// succeeds as well. Therefore, a test can only check the arguments of a
    /// call that reverts. All the mocks of a VM also share one return data
    /// buffer, so the calls are mocked right before the wrapper call that makes
    /// them.
    struct UnderlyingToken {
        vm: TestVM,
        erc721: Erc721,
    }

    impl UnderlyingToken {
        fn new() -> Self {
            let vm = TestVM::default();
            vm.set_contract_address(UNDERLYING);
            let erc721 = Erc721::from(&vm);
            Self { vm, erc721 }
        }

        /// Returns the token, to be called by `sender`.
        fn sender(&mut self, sender: Address) -> &mut Erc721 {
            self.vm.set_sender(sender);
            &mut self.erc721
        }

        /// Asserts that the token has emitted `event`.
        fn assert_emitted(&self, event: &impl SolEvent) {
            assert_emitted(&self.vm, event);
        }

        /// Mocks the answer of the token to the wrapper deployed in
        /// `wrapper_vm` calling `transferFrom(from, to, token_id)`.
        fn mock_transfer_from(
            &mut self,
            wrapper_vm: &TestVM,
            from: Address,
            to: Address,
            token_id: U256,
        ) {
            let wrapper = wrapper_vm.contract_address();
            let answer = self
                .sender(wrapper)
                .transfer_from(from, to, token_id)
                .map(|()| Vec::new())
                .map_err(Vec::<u8>::from);
            let calldata = transferFromCall { from, to, token_id }.abi_encode();
            wrapper_vm.mock_call(UNDERLYING, calldata, U256::ZERO, answer);
        }

        /// Mocks the answer of the token to the wrapper deployed in
        /// `wrapper_vm` calling `safeTransferFrom(from, to, token_id, "")`.
        fn mock_safe_transfer_from(
            &mut self,
            wrapper_vm: &TestVM,
            from: Address,
            to: Address,
            token_id: U256,
        ) {
            let wrapper = wrapper_vm.contract_address();
            let answer = self
                .sender(wrapper)
                .safe_transfer_from_with_data(from, to, token_id, Bytes::new())
                .map(|()| Vec::new())
                .map_err(Vec::<u8>::from);
            let calldata =
                safeTransferFromCall { from, to, token_id, data: Bytes::new() }
                    .abi_encode();
            wrapper_vm.mock_call(UNDERLYING, calldata, U256::ZERO, answer);
        }

        /// Mocks the answer of the token to the wrapper deployed in
        /// `wrapper_vm` calling `ownerOf(token_id)`.
        fn mock_owner_of(&mut self, wrapper_vm: &TestVM, token_id: U256) {
            let wrapper = wrapper_vm.contract_address();
            let answer = self
                .sender(wrapper)
                .owner_of(token_id)
                .map(|owner| ownerOfCall::abi_encode_returns(&owner))
                .map_err(Vec::<u8>::from);
            let calldata = ownerOfCall { token_id }.abi_encode();
            wrapper_vm.mock_static_call(UNDERLYING, calldata, answer);
        }
    }

    #[storage]
    struct Erc721WrapperTestExample {
        wrapper: Erc721Wrapper,
        erc721: Erc721,
    }

    #[public]
    #[implements(IErc721<Error = erc721::Error>, IErc721Wrapper<Error = Error>, IErc165)]
    impl Erc721WrapperTestExample {
        #[constructor]
        fn constructor(&mut self, underlying_token: Address) {
            self.wrapper.constructor(underlying_token);
        }

        fn recover(
            &mut self,
            account: Address,
            token_id: U256,
        ) -> Result<U256, Error> {
            self.wrapper._recover(account, token_id, &mut self.erc721)
        }
    }

    #[public]
    impl IErc721 for Erc721WrapperTestExample {
        type Error = erc721::Error;

        fn balance_of(&self, owner: Address) -> Result<U256, erc721::Error> {
            self.erc721.balance_of(owner)
        }

        fn owner_of(&self, token_id: U256) -> Result<Address, erc721::Error> {
            self.erc721.owner_of(token_id)
        }

        fn safe_transfer_from(
            &mut self,
            from: Address,
            to: Address,
            token_id: U256,
        ) -> Result<(), erc721::Error> {
            self.erc721.safe_transfer_from(from, to, token_id)
        }

        fn safe_transfer_from_with_data(
            &mut self,
            from: Address,
            to: Address,
            token_id: U256,
            data: Bytes,
        ) -> Result<(), erc721::Error> {
            self.erc721.safe_transfer_from_with_data(from, to, token_id, data)
        }

        fn transfer_from(
            &mut self,
            from: Address,
            to: Address,
            token_id: U256,
        ) -> Result<(), erc721::Error> {
            self.erc721.transfer_from(from, to, token_id)
        }

        fn approve(
            &mut self,
            to: Address,
            token_id: U256,
        ) -> Result<(), erc721::Error> {
            self.erc721.approve(to, token_id)
        }

        fn set_approval_for_all(
            &mut self,
            operator: Address,
            approved: bool,
        ) -> Result<(), erc721::Error> {
            self.erc721.set_approval_for_all(operator, approved)
        }

        fn get_approved(
            &self,
            token_id: U256,
        ) -> Result<Address, erc721::Error> {
            self.erc721.get_approved(token_id)
        }

        fn is_approved_for_all(
            &self,
            owner: Address,
            operator: Address,
        ) -> bool {
            self.erc721.is_approved_for_all(owner, operator)
        }
    }

    #[public]
    impl IErc721Wrapper for Erc721WrapperTestExample {
        type Error = Error;

        fn underlying(&self) -> Address {
            self.wrapper.underlying()
        }

        fn deposit_for(
            &mut self,
            account: Address,
            token_ids: Vec<U256>,
        ) -> Result<bool, Error> {
            self.wrapper.deposit_for(account, token_ids, &mut self.erc721)
        }

        fn withdraw_to(
            &mut self,
            account: Address,
            token_ids: Vec<U256>,
        ) -> Result<bool, Error> {
            self.wrapper.withdraw_to(account, token_ids, &mut self.erc721)
        }
    }

    #[public]
    impl IErc721Receiver for Erc721WrapperTestExample {
        fn on_erc721_received(
            &mut self,
            operator: Address,
            from: Address,
            token_id: U256,
            data: Bytes,
        ) -> Result<B32, Vec<u8>> {
            Ok(self.wrapper.on_erc721_received(
                operator,
                from,
                token_id,
                &data,
                &mut self.erc721,
            )?)
        }
    }

    #[public]
    impl IErc165 for Erc721WrapperTestExample {
        fn supports_interface(&self, interface_id: B32) -> bool {
            self.erc721.supports_interface(interface_id)
        }
    }

    #[test]
    fn underlying_works() {
        let vm = TestVM::default();
        let mut contract = Erc721WrapperTestExample::from(&vm);
        vm.set_sender(ALICE);

        contract.constructor(UNDERLYING);

        assert_eq!(contract.underlying(), UNDERLYING);
    }

    // `deposit_for` doesn't check that the underlying token is a contract:
    // calling an address without code succeeds.
    #[test]
    #[ignore = "calling an address without code doesn't revert"]
    fn deposit_for_reverts_when_unsupported_token() {
        let vm = TestVM::default();
        let mut contract = Erc721WrapperTestExample::from(&vm);
        vm.set_sender(ALICE);
        let token_ids = random_token_ids(1);

        let invalid_token = ALICE;
        contract.constructor(invalid_token);

        let err = contract
            .deposit_for(ALICE, token_ids.clone())
            .expect_err("should return Error::UnsupportedToken");

        assert!(matches!(
            err,
            Error::UnsupportedToken(ERC721UnsupportedToken { token }
            ) if token == invalid_token
        ));
    }

    #[test]
    fn deposit_for_reverts_when_nonexistent_token() {
        let vm = TestVM::default();
        let mut contract = Erc721WrapperTestExample::from(&vm);
        let mut underlying = UnderlyingToken::new();
        let wrapper = vm.contract_address();
        vm.set_sender(ALICE);
        let token_ids = random_token_ids(1);

        contract.constructor(UNDERLYING);

        underlying.mock_transfer_from(&vm, ALICE, wrapper, token_ids[0]);
        let err = contract
            .deposit_for(ALICE, token_ids.clone())
            .expect_err("should return Error::InvalidReceiverWithReason");

        let expected_error: Vec<u8> =
            erc721::Error::NonexistentToken(erc721::ERC721NonexistentToken {
                token_id: token_ids[0],
            })
            .into();
        let expected_error = String::from_utf8_lossy(&expected_error);

        assert!(matches!(
            err,
            Error::InvalidReceiverWithReason(erc721::InvalidReceiverWithReason { reason })
                if reason == expected_error
        ));
    }

    #[test]
    fn deposit_for_reverts_when_missing_approval() {
        let vm = TestVM::default();
        let mut contract = Erc721WrapperTestExample::from(&vm);
        let mut underlying = UnderlyingToken::new();
        let wrapper = vm.contract_address();
        vm.set_sender(ALICE);
        let token_ids = random_token_ids(1);

        contract.constructor(UNDERLYING);

        underlying
            .sender(ALICE)
            ._mint(ALICE, token_ids[0])
            .expect("should mint {token_id} for {alice}");

        underlying.mock_transfer_from(&vm, ALICE, wrapper, token_ids[0]);
        let err = contract
            .deposit_for(ALICE, token_ids.clone())
            .expect_err("should return Error::InvalidReceiverWithReason");

        let expected_error: Vec<u8> = erc721::Error::InsufficientApproval(
            erc721::ERC721InsufficientApproval {
                operator: wrapper,
                token_id: token_ids[0],
            },
        )
        .into();
        let expected_error = String::from_utf8_lossy(&expected_error);

        assert!(matches!(
            err,
            Error::InvalidReceiverWithReason(erc721::InvalidReceiverWithReason { reason })
                if reason == expected_error
        ));
    }

    #[test]
    fn deposit_for_reverts_when_wrapped_token_already_exists() {
        let vm = TestVM::default();
        let mut contract = Erc721WrapperTestExample::from(&vm);
        let mut underlying = UnderlyingToken::new();
        let wrapper = vm.contract_address();
        vm.set_sender(ALICE);
        let token_ids = random_token_ids(1);

        contract.constructor(UNDERLYING);

        underlying
            .sender(ALICE)
            ._mint(ALICE, token_ids[0])
            .expect("should mint {token_id} for {alice}");

        underlying
            .sender(ALICE)
            .approve(wrapper, token_ids[0])
            .expect("should approve {token_id} for {contract.address()}");

        // Mint an "unexpected" wrapped token.
        contract
            .erc721
            ._mint(ALICE, token_ids[0])
            .expect("should mint {token_id} for {alice}");

        underlying.mock_transfer_from(&vm, ALICE, wrapper, token_ids[0]);
        let err = contract
            .deposit_for(ALICE, token_ids.clone())
            .expect_err("should return Error::Erc721");

        assert!(matches!(
            err,
            Error::InvalidSender(
                erc721::ERC721InvalidSender { sender }
            ) if sender.is_zero()
        ));
    }

    #[test]
    fn deposit_for_works() {
        let vm = TestVM::default();
        let mut contract = Erc721WrapperTestExample::from(&vm);
        let mut underlying = UnderlyingToken::new();
        let wrapper = vm.contract_address();
        vm.set_sender(ALICE);
        let tokens = 4;
        let token_ids = random_token_ids(tokens);

        contract.constructor(UNDERLYING);

        for &token_id in &token_ids {
            underlying
                .sender(ALICE)
                ._mint(ALICE, token_id)
                .expect("should mint {token_id} for {alice}");

            underlying
                .sender(ALICE)
                .approve(wrapper, token_id)
                .expect("should approve {token_id} for {contract.address()}");
        }

        let initial_balance =
            underlying.sender(ALICE).balance_of(ALICE).unwrap();
        let initial_wrapped_balance =
            contract.erc721.balance_of(ALICE).unwrap();

        let initial_contract_balance =
            underlying.sender(ALICE).balance_of(wrapper).unwrap();

        for &token_id in &token_ids {
            underlying.mock_transfer_from(&vm, ALICE, wrapper, token_id);
        }
        assert!(contract
            .deposit_for(ALICE, token_ids.clone())
            .expect("should deposit"));

        for token_id in token_ids {
            underlying.assert_emitted(&erc721::Transfer {
                from: ALICE,
                to: wrapper,
                token_id,
            });

            assert_emitted(
                &vm,
                &erc721::Transfer { from: Address::ZERO, to: ALICE, token_id },
            );
        }

        assert_eq!(
            underlying.sender(ALICE).balance_of(ALICE).unwrap(),
            initial_balance - U256::from(tokens)
        );

        assert_eq!(
            contract.erc721.balance_of(ALICE).unwrap(),
            initial_wrapped_balance + U256::from(tokens)
        );

        assert_eq!(
            underlying.sender(wrapper).balance_of(wrapper).unwrap(),
            initial_contract_balance + U256::from(tokens)
        );
    }

    #[test]
    fn withdraw_to_reverts_when_invalid_receiver() {
        let vm = TestVM::default();
        let mut contract = Erc721WrapperTestExample::from(&vm);
        let mut underlying = UnderlyingToken::new();
        let wrapper = vm.contract_address();
        vm.set_sender(ALICE);
        let tokens = 4;
        let token_ids = random_token_ids(tokens);

        contract.constructor(UNDERLYING);

        for token_id in &token_ids {
            underlying
                .sender(ALICE)
                ._mint(ALICE, *token_id)
                .expect("should mint {token_id} for {alice}");

            underlying
                .sender(ALICE)
                .approve(wrapper, *token_id)
                .expect("should approve {token_id} for {contract.address()}");
        }

        for &token_id in &token_ids {
            underlying.mock_transfer_from(&vm, ALICE, wrapper, token_id);
        }
        assert!(contract
            .deposit_for(ALICE, token_ids.clone())
            .expect("should deposit"));

        underlying.mock_safe_transfer_from(
            &vm,
            wrapper,
            Address::ZERO,
            token_ids[0],
        );
        let err = contract
            .withdraw_to(Address::ZERO, token_ids.clone())
            .expect_err("should return Error::InvalidReceiverWithReason");

        let expected_error: Vec<u8> =
            erc721::Error::InvalidReceiver(erc721::ERC721InvalidReceiver {
                receiver: Address::ZERO,
            })
            .into();
        let expected_error = String::from_utf8_lossy(&expected_error);

        assert!(matches!(
            err,
            Error::InvalidReceiverWithReason(
                erc721::InvalidReceiverWithReason { reason }
            ) if reason == expected_error
        ));
    }

    #[test]
    fn withdraw_to_reverts_when_nonexistent_token() {
        let vm = TestVM::default();
        let mut contract = Erc721WrapperTestExample::from(&vm);
        vm.set_sender(ALICE);
        let tokens = 1;
        let token_ids = random_token_ids(tokens);

        contract.constructor(UNDERLYING);

        let err = contract
            .withdraw_to(ALICE, token_ids.clone())
            .expect_err("should return Error::Erc721");

        assert!(matches!(
            err,
            Error::NonexistentToken(
                erc721::ERC721NonexistentToken { token_id },
            ) if token_id == token_ids[0]
        ));
    }

    #[test]
    fn withdraw_to_reverts_when_insufficient_approval() {
        let vm = TestVM::default();
        let mut contract = Erc721WrapperTestExample::from(&vm);
        let mut underlying = UnderlyingToken::new();
        let wrapper = vm.contract_address();
        vm.set_sender(ALICE);
        let tokens = 1;
        let token_ids = random_token_ids(tokens);

        contract.constructor(UNDERLYING);

        underlying
            .sender(ALICE)
            ._mint(ALICE, token_ids[0])
            .expect("should mint {token_id} for {alice}");

        underlying
            .sender(ALICE)
            .approve(wrapper, token_ids[0])
            .expect("should approve {token_id} for {contract.address()}");

        underlying.mock_transfer_from(&vm, ALICE, wrapper, token_ids[0]);
        assert!(contract
            .deposit_for(ALICE, token_ids.clone())
            .expect("should deposit"));

        vm.set_sender(BOB);
        let err = contract
            .withdraw_to(ALICE, token_ids.clone())
            .expect_err("should return Error::Erc721");

        assert!(matches!(
            err,
            Error::InsufficientApproval(
                erc721::ERC721InsufficientApproval { token_id, operator},
            ) if token_id == token_ids[0] && operator == BOB
        ));
    }

    #[test]
    fn withdraw_to_works() {
        let vm = TestVM::default();
        let mut contract = Erc721WrapperTestExample::from(&vm);
        let mut underlying = UnderlyingToken::new();
        let wrapper = vm.contract_address();
        vm.set_sender(ALICE);
        let tokens = 4;
        let token_ids = random_token_ids(tokens);

        contract.constructor(UNDERLYING);

        for token_id in &token_ids {
            underlying
                .sender(ALICE)
                ._mint(ALICE, *token_id)
                .expect("should mint {token_id} for {alice}");

            underlying
                .sender(ALICE)
                .approve(wrapper, *token_id)
                .expect("should approve {token_id} for {contract.address()}");
        }

        for &token_id in &token_ids {
            underlying.mock_transfer_from(&vm, ALICE, wrapper, token_id);
        }
        assert!(contract
            .deposit_for(ALICE, token_ids.clone())
            .expect("should deposit"));

        let initial_balance =
            underlying.sender(ALICE).balance_of(ALICE).unwrap();
        let initial_wrapped_balance =
            contract.erc721.balance_of(ALICE).unwrap();

        let initial_contract_balance =
            underlying.sender(ALICE).balance_of(wrapper).unwrap();

        for &token_id in &token_ids {
            underlying.mock_safe_transfer_from(&vm, wrapper, ALICE, token_id);
        }
        assert!(contract
            .withdraw_to(ALICE, token_ids.clone())
            .expect("should withdraw"));

        for token_id in token_ids {
            underlying.assert_emitted(&erc721::Transfer {
                from: wrapper,
                to: ALICE,
                token_id,
            });

            assert_emitted(
                &vm,
                &erc721::Transfer { from: ALICE, to: Address::ZERO, token_id },
            );
        }

        assert_eq!(
            underlying.sender(ALICE).balance_of(ALICE).unwrap(),
            initial_balance + U256::from(tokens)
        );

        assert_eq!(
            contract.erc721.balance_of(ALICE).unwrap(),
            initial_wrapped_balance - U256::from(tokens)
        );

        assert_eq!(
            underlying.sender(wrapper).balance_of(wrapper).unwrap(),
            initial_contract_balance - U256::from(tokens)
        );
    }

    #[test]
    fn on_erc721_received_reverts_when_sender_is_unsupported_token() {
        let vm = TestVM::default();
        let mut contract = Erc721WrapperTestExample::from(&vm);
        let token_id = random_token_ids(1)[0];
        vm.set_sender(ALICE);

        contract.constructor(UNDERLYING);

        let invalid_operator = ALICE;

        vm.set_sender(invalid_operator);
        let err = contract
            .on_erc721_received(
                invalid_operator,
                ALICE,
                token_id,
                vec![].into(),
            )
            .expect_err("should return Error::UnsupportedToken");

        assert_eq!(
            err,
            ERC721UnsupportedToken { token: invalid_operator }.abi_encode()
        );
    }

    #[test]
    fn on_erc721_received_reverts_when_wrapped_token_already_exists() {
        let vm = TestVM::default();
        let mut contract = Erc721WrapperTestExample::from(&vm);
        let token_id = random_token_ids(1)[0];
        vm.set_sender(ALICE);

        contract.constructor(UNDERLYING);

        // Mint an "unexpected" wrapped token.
        contract
            .erc721
            ._mint(ALICE, token_id)
            .expect("should mint {token_id} for {alice}");

        let operator = ALICE;

        vm.set_sender(UNDERLYING);
        let err = contract
            .on_erc721_received(operator, ALICE, token_id, vec![].into())
            .expect_err("should return Error::Erc721");

        assert_eq!(
            err,
            erc721::ERC721InvalidSender { sender: Address::ZERO }.abi_encode()
        );
    }

    #[test]
    fn on_erc721_received_works() {
        let vm = TestVM::default();
        let mut contract = Erc721WrapperTestExample::from(&vm);
        let token_id = random_token_ids(1)[0];
        vm.set_sender(ALICE);

        contract.constructor(UNDERLYING);

        let initial_wrapped_balance =
            contract.erc721.balance_of(ALICE).unwrap();

        let operator = ALICE;
        vm.set_sender(UNDERLYING);
        let interface_id = contract
            .on_erc721_received(operator, ALICE, token_id, vec![].into())
            .expect("should handle ERC721Received");

        assert_eq!(interface_id, RECEIVER_FN_SELECTOR);

        assert_emitted(
            &vm,
            &erc721::Transfer { from: Address::ZERO, to: ALICE, token_id },
        );

        assert_eq!(
            contract.erc721.balance_of(ALICE).unwrap(),
            initial_wrapped_balance + uint!(1_U256)
        );
    }

    #[test]
    fn recover_reverts_when_invalid_token() {
        let vm = TestVM::default();
        let mut contract = Erc721WrapperTestExample::from(&vm);
        let token_id = random_token_ids(1)[0];
        vm.set_sender(ALICE);
        let invalid_token_address = ALICE;

        contract.constructor(invalid_token_address);

        let err = contract
            .recover(ALICE, token_id)
            .expect_err("should return Error::Erc721FailedOperation");

        assert!(matches!(
            err,
            Error::Erc721FailedOperation(Erc721FailedOperation { token })
                if token == invalid_token_address
        ));
    }

    #[test]
    fn recover_reverts_when_incorrect_owner() {
        let vm = TestVM::default();
        let mut contract = Erc721WrapperTestExample::from(&vm);
        let mut underlying = UnderlyingToken::new();
        let wrapper = vm.contract_address();
        vm.set_sender(ALICE);
        let token_id = random_token_ids(1)[0];

        contract.constructor(UNDERLYING);

        underlying
            .sender(ALICE)
            ._mint(ALICE, token_id)
            .expect("should mint {token_id} for {alice}");

        underlying.mock_owner_of(&vm, token_id);
        let err = contract
            .recover(ALICE, token_id)
            .expect_err("should return Error::Erc721");

        assert!(matches!(
            err,
            Error::IncorrectOwner(
                erc721::ERC721IncorrectOwner { sender, token_id: t_id, owner },
            ) if sender == wrapper && t_id == token_id && owner == ALICE
        ));
    }

    #[test]
    fn recover_reverts_when_wrapped_token_already_exists() {
        let vm = TestVM::default();
        let mut contract = Erc721WrapperTestExample::from(&vm);
        let mut underlying = UnderlyingToken::new();
        let wrapper = vm.contract_address();
        vm.set_sender(ALICE);
        let token_id = random_token_ids(1)[0];

        contract.constructor(UNDERLYING);

        underlying
            .sender(ALICE)
            ._mint(ALICE, token_id)
            .expect("should mint {token_id} for {alice}");

        underlying
            .sender(ALICE)
            .transfer_from(ALICE, wrapper, token_id)
            .expect(
            "should transfer {token_id} from {alice} to {contract.address()}",
        );

        // Mint an "unexpected" wrapped token.
        contract
            .erc721
            ._mint(ALICE, token_id)
            .expect("should mint {token_id} for {alice}");

        underlying.mock_owner_of(&vm, token_id);
        let err = contract
            .recover(ALICE, token_id)
            .expect_err("should return Error::Erc721");

        assert!(matches!(
            err,
            Error::InvalidSender(
                erc721::ERC721InvalidSender { sender }
            ) if sender.is_zero()
        ));
    }

    #[test]
    fn recover_works() {
        let vm = TestVM::default();
        let mut contract = Erc721WrapperTestExample::from(&vm);
        let mut underlying = UnderlyingToken::new();
        let wrapper = vm.contract_address();
        vm.set_sender(ALICE);
        let token_id = random_token_ids(1)[0];

        contract.constructor(UNDERLYING);

        underlying
            .sender(ALICE)
            ._mint(ALICE, token_id)
            .expect("should mint {token_id} for {alice}");

        underlying
            .sender(ALICE)
            .transfer_from(ALICE, wrapper, token_id)
            .expect(
            "should transfer {token_id} from {alice} to {contract.address()}",
        );

        let initial_wrapped_balance =
            contract.erc721.balance_of(ALICE).unwrap();

        underlying.mock_owner_of(&vm, token_id);
        contract
            .recover(ALICE, token_id)
            .expect("should recover {token_id} for {alice}");

        let wrapped_balance = contract.erc721.balance_of(ALICE).unwrap();

        assert_eq!(wrapped_balance, initial_wrapped_balance + uint!(1_U256));

        assert_emitted(
            &vm,
            &erc721::Transfer { from: Address::ZERO, to: ALICE, token_id },
        );

        assert_eq!(
            contract.erc721.balance_of(ALICE).unwrap(),
            initial_wrapped_balance + uint!(1_U256)
        );
    }
}
