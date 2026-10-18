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
use stylus_sdk::{abi::Bytes, prelude::*, storage::StorageAddress};

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
        let vm = self.vm().clone();

        for token_id in token_ids {
            // This is an "unsafe" transfer that doesn't call any hook on
            // the receiver. With [`IErc721Wrapper::underlying()`] being trusted
            // (by design of this contract) and no other contracts expected to
            // be called from there, we are safe.
            match underlying.transfer_from(
                &vm,
                Call::new_mutating(self),
                sender,
                contract_address,
                token_id,
            ) {
                Ok(()) => (),
                Err(e) => {
                    if let stylus_sdk::stylus_core::calls::errors::Error::Revert(ref reason) = e {
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
        let contract_address = self.vm().contract_address();
        let underlying = Erc721Interface::new(self.underlying());
        let vm = self.vm().clone();

        for token_id in token_ids {
            // Setting the `auth` argument enables the `_is_authorized` check
            // which verifies that the token exists (from != 0).
            // Therefore, it is not needed to verify that the return value is
            // not 0 here.
            erc721._update(Address::ZERO, token_id, sender)?;
            match underlying.safe_transfer_from(
                &vm,
                Call::new_mutating(self),
                contract_address,
                account,
                token_id,
                vec![].into(),
            ) {
                Ok(()) => (),
                Err(e) => {
                    if let stylus_sdk::stylus_core::calls::errors::Error::Revert(ref reason) = e {
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
    /// * [`Error::UnsupportedToken`] - If the caller is not the underlying
    ///   token.
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
    ///   [`Erc721`]  or the contract fails to execute the call.
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

use stylus_sdk::prelude::errors::MethodError;
#[cfg(test)]
mod tests {
    use crate::test_support::{
        mock_call, mock_call_with_encoded_args, mock_code, mock_static_call,
        test_address, TestContract as TestVMContract,
    };
    use alloy_primitives::uint;
    use alloy_sol_types::{SolError, SolValue};
    use stylus_sdk::abi::Bytes;

    use super::*;
    use crate::{
        token::erc721::{self, IErc721},
        utils::introspection::erc165::IErc165,
    };

    pub(crate) fn random_token_ids(size: usize) -> Vec<U256> {
        (0..size).map(U256::from).collect()
    }

    fn mock_underlying_transfer_from(
        contract: &TestVMContract<Erc721WrapperTestExample>,
        token: Address,
        from: Address,
        to: Address,
        token_id: U256,
        result: Result<Vec<u8>, Vec<u8>>,
    ) {
        mock_code(contract.vm(), token);
        mock_call(
            contract.vm(),
            token,
            "transferFrom(address,address,uint256)",
            (from, to, token_id),
            result,
        );
    }

    fn mock_underlying_safe_transfer_from(
        contract: &TestVMContract<Erc721WrapperTestExample>,
        token: Address,
        from: Address,
        to: Address,
        token_id: U256,
        result: Result<Vec<u8>, Vec<u8>>,
    ) {
        mock_code(contract.vm(), token);
        let args = (from, to, token_id, Bytes::new()).abi_encode_params();
        mock_call_with_encoded_args(
            contract.vm(),
            token,
            "safeTransferFrom(address,address,uint256,bytes)",
            &args,
            result,
        );
    }

    fn mock_underlying_owner_of(
        contract: &TestVMContract<Erc721WrapperTestExample>,
        token: Address,
        token_id: U256,
        owner: Address,
    ) {
        mock_code(contract.vm(), token);
        mock_static_call(
            contract.vm(),
            token,
            "ownerOf(uint256)",
            (token_id,),
            Ok(owner.abi_encode()),
        );
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

    unsafe impl TopLevelStorage for Erc721WrapperTestExample {}

    #[test]
    fn underlying_works() {
        let contract = TestVMContract::<Erc721WrapperTestExample>::new(
            stringify!(contract),
        );
        let erc721_contract =
            TestVMContract::<Erc721>::new(stringify!(erc721_contract));
        let alice = test_address(stringify!(alice));

        let erc721_address = erc721_contract.address();

        contract.sender(alice).constructor(erc721_address);

        assert_eq!(contract.sender(alice).underlying(), erc721_address);
    }

    #[test]
    fn deposit_for_reverts_when_underlying_transfer_fails() {
        let contract = TestVMContract::<Erc721WrapperTestExample>::new(
            stringify!(contract),
        );
        let alice = test_address(stringify!(alice));

        let token_ids = random_token_ids(1);

        let invalid_token = test_address(stringify!(invalid_token));
        contract.sender(alice).constructor(invalid_token);
        mock_underlying_transfer_from(
            &contract,
            invalid_token,
            alice,
            contract.address(),
            token_ids[0],
            Err(Vec::new()),
        );
        let err =
            contract.sender(alice).deposit_for(alice, token_ids).expect_err(
                "a failed underlying transfer must not mint a wrapper token",
            );
        assert!(matches!(
            err,
            Error::Erc721FailedOperation(Erc721FailedOperation { token })
                if token == invalid_token
        ));
    }

    #[test]
    fn deposit_for_reverts_when_underlying_has_no_code() {
        let contract = TestVMContract::<Erc721WrapperTestExample>::new(
            stringify!(contract),
        );
        let alice = test_address(stringify!(alice));
        let underlying = test_address(stringify!(underlying));
        let token_ids = random_token_ids(1);

        contract.sender(alice).constructor(underlying);

        let err = contract
            .sender(alice)
            .deposit_for(alice, token_ids)
            .expect_err("an address without code cannot be an underlying token");
        assert!(matches!(
            err,
            Error::UnsupportedToken(ERC721UnsupportedToken { token })
                if token == underlying
        ));
    }

    #[test]
    fn withdraw_to_reverts_when_underlying_has_no_code() {
        let contract = TestVMContract::<Erc721WrapperTestExample>::new(
            stringify!(contract),
        );
        let alice = test_address(stringify!(alice));
        let underlying = test_address(stringify!(underlying));
        let token_id = random_token_ids(1)[0];

        contract.sender(alice).constructor(underlying);
        contract
            .sender(alice)
            .erc721
            ._mint(alice, token_id)
            .expect("should mint wrapped token for test");

        let err = contract
            .sender(alice)
            .withdraw_to(alice, vec![token_id])
            .expect_err("withdrawal must not burn without an underlying token");
        assert!(matches!(
            err,
            Error::UnsupportedToken(ERC721UnsupportedToken { token })
                if token == underlying
        ));
        assert_eq!(
            contract
                .sender(alice)
                .owner_of(token_id)
                .expect("wrapped token should remain owned after failed withdrawal"),
            alice
        );
    }

    #[test]
    fn on_erc721_received_reverts_when_underlying_has_no_code() {
        let contract = TestVMContract::<Erc721WrapperTestExample>::new(
            stringify!(contract),
        );
        let alice = test_address(stringify!(alice));
        let underlying = test_address(stringify!(underlying));
        let token_id = random_token_ids(1)[0];

        contract.sender(alice).constructor(underlying);

        let err = contract
            .sender(underlying)
            .on_erc721_received(
                underlying,
                alice,
                token_id,
                vec![].into(),
            )
            .expect_err("an address without code cannot mint a wrapped token");
        assert_eq!(
            err,
            ERC721UnsupportedToken { token: underlying }.abi_encode()
        );
    }

    #[test]
    fn deposit_for_reverts_when_nonexistent_token() {
        let contract = TestVMContract::<Erc721WrapperTestExample>::new(
            stringify!(contract),
        );
        let erc721_contract =
            TestVMContract::<Erc721>::new(stringify!(erc721_contract));
        let alice = test_address(stringify!(alice));

        let token_ids = random_token_ids(1);

        contract.sender(alice).constructor(erc721_contract.address());

        let transfer_error: Vec<u8> =
            erc721::Error::NonexistentToken(erc721::ERC721NonexistentToken {
                token_id: token_ids[0],
            })
            .into();
        mock_underlying_transfer_from(
            &contract,
            erc721_contract.address(),
            alice,
            contract.address(),
            token_ids[0],
            Err(transfer_error),
        );

        let err = contract
            .sender(alice)
            .deposit_for(alice, token_ids.clone())
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
        let contract = TestVMContract::<Erc721WrapperTestExample>::new(
            stringify!(contract),
        );
        let erc721_contract =
            TestVMContract::<Erc721>::new(stringify!(erc721_contract));
        let alice = test_address(stringify!(alice));

        let token_ids = random_token_ids(1);

        contract.sender(alice).constructor(erc721_contract.address());

        erc721_contract
            .sender(alice)
            ._mint(alice, token_ids[0])
            .expect("should mint {token_id} for {alice}");

        let transfer_error: Vec<u8> = erc721::Error::InsufficientApproval(
            erc721::ERC721InsufficientApproval {
                operator: contract.address(),
                token_id: token_ids[0],
            },
        )
        .into();
        mock_underlying_transfer_from(
            &contract,
            erc721_contract.address(),
            alice,
            contract.address(),
            token_ids[0],
            Err(transfer_error),
        );

        let err = contract
            .sender(alice)
            .deposit_for(alice, token_ids.clone())
            .expect_err("should return Error::InvalidReceiverWithReason");

        let expected_error: Vec<u8> = erc721::Error::InsufficientApproval(
            erc721::ERC721InsufficientApproval {
                operator: contract.address(),
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
        let contract = TestVMContract::<Erc721WrapperTestExample>::new(
            stringify!(contract),
        );
        let erc721_contract =
            TestVMContract::<Erc721>::new(stringify!(erc721_contract));
        let alice = test_address(stringify!(alice));

        let token_ids = random_token_ids(1);

        contract.sender(alice).constructor(erc721_contract.address());

        erc721_contract
            .sender(alice)
            ._mint(alice, token_ids[0])
            .expect("should mint {token_id} for {alice}");

        erc721_contract
            .sender(alice)
            .approve(contract.address(), token_ids[0])
            .expect("should approve {token_id} for {contract.address()}");

        mock_underlying_transfer_from(
            &contract,
            erc721_contract.address(),
            alice,
            contract.address(),
            token_ids[0],
            Ok(Vec::new()),
        );

        // Mint an "unexpected" wrapped token.
        contract
            .sender(alice)
            .erc721
            ._mint(alice, token_ids[0])
            .expect("should mint {token_id} for {alice}");

        let err = contract
            .sender(alice)
            .deposit_for(alice, token_ids.clone())
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
        let contract = TestVMContract::<Erc721WrapperTestExample>::new(
            stringify!(contract),
        );
        let erc721_contract =
            TestVMContract::<Erc721>::new(stringify!(erc721_contract));
        let alice = test_address(stringify!(alice));

        let tokens = 4;
        let token_ids = random_token_ids(tokens);

        contract.sender(alice).constructor(erc721_contract.address());

        mock_underlying_transfer_from(
            &contract,
            erc721_contract.address(),
            alice,
            contract.address(),
            token_ids[0],
            Ok(Vec::new()),
        );

        for &token_id in &token_ids {
            erc721_contract
                .sender(alice)
                ._mint(alice, token_id)
                .expect("should mint {token_id} for {alice}");

            erc721_contract
                .sender(alice)
                .approve(contract.address(), token_id)
                .expect("should approve {token_id} for {contract.address()}");

            mock_underlying_transfer_from(
                &contract,
                erc721_contract.address(),
                alice,
                contract.address(),
                token_id,
                Ok(Vec::new()),
            );
        }

        let initial_balance =
            erc721_contract.sender(alice).balance_of(alice).unwrap();
        let initial_wrapped_balance =
            contract.sender(alice).erc721.balance_of(alice).unwrap();

        let initial_contract_balance = erc721_contract
            .sender(alice)
            .balance_of(contract.address())
            .unwrap();

        assert!(contract
            .sender(alice)
            .deposit_for(alice, token_ids.clone())
            .expect("should deposit"));

        for token_id in token_ids {
            contract.assert_emitted(&erc721::Transfer {
                from: Address::ZERO,
                to: alice,
                token_id,
            });
        }

        assert_eq!(
            contract.sender(alice).erc721.balance_of(alice).unwrap(),
            initial_wrapped_balance + U256::from(tokens)
        );

        let _ = (initial_balance, initial_contract_balance);
    }

    #[test]
    fn withdraw_to_reverts_when_invalid_receiver() {
        let contract = TestVMContract::<Erc721WrapperTestExample>::new(
            stringify!(contract),
        );
        let erc721_contract =
            TestVMContract::<Erc721>::new(stringify!(erc721_contract));
        let alice = test_address(stringify!(alice));

        let tokens = 4;
        let token_ids = random_token_ids(tokens);

        contract.sender(alice).constructor(erc721_contract.address());

        for token_id in &token_ids {
            erc721_contract
                .sender(alice)
                ._mint(alice, *token_id)
                .expect("should mint {token_id} for {alice}");

            erc721_contract
                .sender(alice)
                .approve(contract.address(), *token_id)
                .expect("should approve {token_id} for {contract.address()}");

            mock_underlying_transfer_from(
                &contract,
                erc721_contract.address(),
                alice,
                contract.address(),
                *token_id,
                Ok(Vec::new()),
            );
        }

        let withdrawal_error: Vec<u8> =
            erc721::Error::InvalidReceiver(erc721::ERC721InvalidReceiver {
                receiver: Address::ZERO,
            })
            .into();
        for token_id in &token_ids {
            mock_underlying_safe_transfer_from(
                &contract,
                erc721_contract.address(),
                contract.address(),
                Address::ZERO,
                *token_id,
                Err(withdrawal_error.clone()),
            );
        }

        assert!(contract
            .sender(alice)
            .deposit_for(alice, token_ids.clone())
            .expect("should deposit"));

        let err = contract
            .sender(alice)
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
        let contract = TestVMContract::<Erc721WrapperTestExample>::new(
            stringify!(contract),
        );
        let erc721_contract =
            TestVMContract::<Erc721>::new(stringify!(erc721_contract));
        let alice = test_address(stringify!(alice));

        let tokens = 1;
        let token_ids = random_token_ids(tokens);

        contract.sender(alice).constructor(erc721_contract.address());

        let err = contract
            .sender(alice)
            .withdraw_to(alice, token_ids.clone())
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
        let contract = TestVMContract::<Erc721WrapperTestExample>::new(
            stringify!(contract),
        );
        let erc721_contract =
            TestVMContract::<Erc721>::new(stringify!(erc721_contract));
        let alice = test_address(stringify!(alice));
        let bob = test_address(stringify!(bob));

        let tokens = 1;
        let token_ids = random_token_ids(tokens);

        contract.sender(alice).constructor(erc721_contract.address());

        erc721_contract
            .sender(alice)
            ._mint(alice, token_ids[0])
            .expect("should mint {token_id} for {alice}");

        erc721_contract
            .sender(alice)
            .approve(contract.address(), token_ids[0])
            .expect("should approve {token_id} for {contract.address()}");

        mock_underlying_transfer_from(
            &contract,
            erc721_contract.address(),
            alice,
            contract.address(),
            token_ids[0],
            Ok(Vec::new()),
        );

        assert!(contract
            .sender(alice)
            .deposit_for(alice, token_ids.clone())
            .expect("should deposit"));

        let err = contract
            .sender(bob)
            .withdraw_to(alice, token_ids.clone())
            .expect_err("should return Error::Erc721");

        assert!(matches!(
            err,
            Error::InsufficientApproval(
                erc721::ERC721InsufficientApproval { token_id, operator},
            ) if token_id == token_ids[0] && operator == bob
        ));
    }

    #[test]
    fn withdraw_to_works() {
        let contract = TestVMContract::<Erc721WrapperTestExample>::new(
            stringify!(contract),
        );
        let erc721_contract =
            TestVMContract::<Erc721>::new(stringify!(erc721_contract));
        let alice = test_address(stringify!(alice));

        let tokens = 4;
        let token_ids = random_token_ids(tokens);

        contract.sender(alice).constructor(erc721_contract.address());

        for token_id in &token_ids {
            erc721_contract
                .sender(alice)
                ._mint(alice, *token_id)
                .expect("should mint {token_id} for {alice}");

            erc721_contract
                .sender(alice)
                .approve(contract.address(), *token_id)
                .expect("should approve {token_id} for {contract.address()}");

            mock_underlying_transfer_from(
                &contract,
                erc721_contract.address(),
                alice,
                contract.address(),
                *token_id,
                Ok(Vec::new()),
            );
        }

        for token_id in &token_ids {
            mock_underlying_safe_transfer_from(
                &contract,
                erc721_contract.address(),
                contract.address(),
                alice,
                *token_id,
                Ok(Vec::new()),
            );
        }

        assert!(contract
            .sender(alice)
            .deposit_for(alice, token_ids.clone())
            .expect("should deposit"));

        let initial_balance =
            erc721_contract.sender(alice).balance_of(alice).unwrap();
        let initial_wrapped_balance =
            contract.sender(alice).erc721.balance_of(alice).unwrap();

        let initial_contract_balance = erc721_contract
            .sender(alice)
            .balance_of(contract.address())
            .unwrap();

        assert!(contract
            .sender(alice)
            .withdraw_to(alice, token_ids.clone())
            .expect("should withdraw"));

        for token_id in token_ids {
            contract.assert_emitted(&erc721::Transfer {
                from: alice,
                to: Address::ZERO,
                token_id,
            });
        }

        assert_eq!(
            contract.sender(alice).erc721.balance_of(alice).unwrap(),
            initial_wrapped_balance - U256::from(tokens)
        );

        let _ = (initial_balance, initial_contract_balance);
    }

    #[test]
    fn on_erc721_received_reverts_when_sender_is_unsupported_token() {
        let contract = TestVMContract::<Erc721WrapperTestExample>::new(
            stringify!(contract),
        );
        let erc721_contract =
            TestVMContract::<Erc721>::new(stringify!(erc721_contract));
        let alice = test_address(stringify!(alice));

        let token_id = random_token_ids(1)[0];

        contract.sender(alice).constructor(erc721_contract.address());

        let invalid_operator = alice;

        let err = contract
            .sender(invalid_operator)
            .on_erc721_received(
                invalid_operator,
                alice,
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
        let contract = TestVMContract::<Erc721WrapperTestExample>::new(
            stringify!(contract),
        );
        let erc721_contract =
            TestVMContract::<Erc721>::new(stringify!(erc721_contract));
        let alice = test_address(stringify!(alice));

        let token_id = random_token_ids(1)[0];

        contract.sender(alice).constructor(erc721_contract.address());

        // Mint an "unexpected" wrapped token.
        contract
            .sender(alice)
            .erc721
            ._mint(alice, token_id)
            .expect("should mint {token_id} for {alice}");

        let operator = alice;

        let err = contract
            .sender(erc721_contract.address())
            .on_erc721_received(operator, alice, token_id, vec![].into())
            .expect_err("should return Error::Erc721");

        assert_eq!(
            err,
            erc721::ERC721InvalidSender { sender: Address::ZERO }.abi_encode()
        );
    }

    #[test]
    fn on_erc721_received_works() {
        let contract = TestVMContract::<Erc721WrapperTestExample>::new(
            stringify!(contract),
        );
        let erc721_contract =
            TestVMContract::<Erc721>::new(stringify!(erc721_contract));
        let alice = test_address(stringify!(alice));

        let token_id = random_token_ids(1)[0];

        contract.sender(alice).constructor(erc721_contract.address());

        let initial_wrapped_balance =
            contract.sender(alice).erc721.balance_of(alice).unwrap();

        let operator = alice;
        let interface_id = contract
            .sender(erc721_contract.address())
            .on_erc721_received(operator, alice, token_id, vec![].into())
            .expect("should handle ERC721Received");

        assert_eq!(interface_id, RECEIVER_FN_SELECTOR);

        contract.assert_emitted(&erc721::Transfer {
            from: Address::ZERO,
            to: alice,
            token_id,
        });

        assert_eq!(
            contract.sender(alice).erc721.balance_of(alice).unwrap(),
            initial_wrapped_balance + uint!(1_U256)
        );
    }

    #[test]
    fn recover_reverts_when_invalid_token() {
        let contract = TestVMContract::<Erc721WrapperTestExample>::new(
            stringify!(contract),
        );
        let alice = test_address(stringify!(alice));

        let token_id = random_token_ids(1)[0];
        let invalid_token_address = alice;

        contract.sender(alice).constructor(invalid_token_address);

        mock_code(contract.vm(), invalid_token_address);
        mock_static_call(
            contract.vm(),
            invalid_token_address,
            "ownerOf(uint256)",
            (token_id,),
            Ok(Vec::new()),
        );

        let err = contract
            .sender(alice)
            .recover(alice, token_id)
            .expect_err("should return Error::Erc721FailedOperation");

        assert!(matches!(
            err,
            Error::Erc721FailedOperation(Erc721FailedOperation { token })
                if token == invalid_token_address
        ));
    }

    #[test]
    fn recover_reverts_when_incorrect_owner() {
        let contract = TestVMContract::<Erc721WrapperTestExample>::new(
            stringify!(contract),
        );
        let erc721_contract =
            TestVMContract::<Erc721>::new(stringify!(erc721_contract));
        let alice = test_address(stringify!(alice));

        let token_id = random_token_ids(1)[0];

        contract.sender(alice).constructor(erc721_contract.address());

        mock_underlying_owner_of(
            &contract,
            erc721_contract.address(),
            token_id,
            alice,
        );

        let err = contract
            .sender(alice)
            .recover(alice, token_id)
            .expect_err("should return Error::Erc721");

        assert!(matches!(
            err,
            Error::IncorrectOwner(
                erc721::ERC721IncorrectOwner { sender, token_id: t_id, owner },
            ) if sender == contract.address() && t_id == token_id && owner == alice
        ));
    }

    #[test]
    fn recover_reverts_when_wrapped_token_already_exists() {
        let contract = TestVMContract::<Erc721WrapperTestExample>::new(
            stringify!(contract),
        );
        let erc721_contract =
            TestVMContract::<Erc721>::new(stringify!(erc721_contract));
        let alice = test_address(stringify!(alice));

        let token_id = random_token_ids(1)[0];

        contract.sender(alice).constructor(erc721_contract.address());

        mock_underlying_owner_of(
            &contract,
            erc721_contract.address(),
            token_id,
            contract.address(),
        );

        // Mint an "unexpected" wrapped token.
        contract
            .sender(alice)
            .erc721
            ._mint(alice, token_id)
            .expect("should mint {token_id} for {alice}");

        let err = contract
            .sender(alice)
            .recover(alice, token_id)
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
        let contract = TestVMContract::<Erc721WrapperTestExample>::new(
            stringify!(contract),
        );
        let erc721_contract =
            TestVMContract::<Erc721>::new(stringify!(erc721_contract));
        let alice = test_address(stringify!(alice));

        let token_id = random_token_ids(1)[0];

        contract.sender(alice).constructor(erc721_contract.address());

        mock_underlying_owner_of(
            &contract,
            erc721_contract.address(),
            token_id,
            contract.address(),
        );

        let initial_wrapped_balance =
            contract.sender(alice).erc721.balance_of(alice).unwrap();

        contract
            .sender(alice)
            .recover(alice, token_id)
            .expect("should recover {token_id} for {alice}");

        let wrapped_balance =
            contract.sender(alice).erc721.balance_of(alice).unwrap();

        assert_eq!(wrapped_balance, initial_wrapped_balance + uint!(1_U256));

        contract.assert_emitted(&erc721::Transfer {
            from: Address::ZERO,
            to: alice,
            token_id,
        });

        assert_eq!(
            contract.sender(alice).erc721.balance_of(alice).unwrap(),
            initial_wrapped_balance + uint!(1_U256)
        );
    }
}
