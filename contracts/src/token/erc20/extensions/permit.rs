//! Permit Contract.
//!
//! Extension of the ERC-20 standard allowing approvals to be made
//! via signatures, as defined in the [ERC].
//!
//! Adds the `permit` method, which can be used to change an account’s
//! ERC20 allowance (see [`crate::token::erc20::IErc20::allowance`])
//! by presenting a message signed by the account.
//! By not relying on [`erc20::IErc20::approve`],
//! the token holder account doesn’t need to send a transaction,
//! and thus is not required to hold Ether at all.
//!
//! [ERC]: https://eips.ethereum.org/EIPS/eip-2612

use alloc::{vec, vec::Vec};

use alloy_primitives::{aliases::B32, keccak256, Address, B256, U256, U8};
use alloy_sol_types::SolType;
use stylus_sdk::{
    function_selector, prelude::*, stylus_core::calls::errors::MethodError,
};

use crate::{
    token::erc20::{self, Erc20},
    utils::{
        cryptography::eip712::IEip712,
        nonces::{INonces, Nonces},
        precompiles::{
            primitives::ecrecover::{
                self, ECDSAInvalidSignature, ECDSAInvalidSignatureS,
            },
            Precompiles,
        },
    },
};

const PERMIT_TYPEHASH: [u8; 32] =
    keccak_const::Keccak256::new()
        .update(b"Permit(address owner,address spender,uint256 value,uint256 nonce,uint256 deadline)")
        .finalize();

pub use sol::*;
#[cfg_attr(coverage_nightly, coverage(off))]
mod sol {
    use alloy_sol_macro::sol;

    pub(crate) type StructHashTuple = sol! {
        tuple(bytes32, address, address, uint256, uint256, uint256)
    };

    sol! {
        /// Indicates an error related to the fact that
        /// permit deadline has expired.
        #[derive(Debug)]
        #[allow(missing_docs)]
        error ERC2612ExpiredSignature(uint256 deadline);

        /// Indicates an error related to the issue about mismatched signature.
        #[derive(Debug)]
        #[allow(missing_docs)]
        error ERC2612InvalidSigner(address signer, address owner);
    }
}

/// A Permit error.
#[derive(SolidityError, Debug)]
pub enum Error {
    /// Indicates an error related to the fact that
    /// permit deadline has expired.
    ExpiredSignature(ERC2612ExpiredSignature),
    /// Indicates an error related to the issue about mismatched signature.
    InvalidSigner(ERC2612InvalidSigner),
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
    /// The signature derives the [`Address::ZERO`].
    InvalidSignature(ECDSAInvalidSignature),
    /// The signature has an `S` value that is in the upper half order.
    InvalidSignatureS(ECDSAInvalidSignatureS),
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

impl From<ecrecover::Error> for Error {
    fn from(value: ecrecover::Error) -> Self {
        match value {
            ecrecover::Error::InvalidSignature(e) => Error::InvalidSignature(e),
            ecrecover::Error::InvalidSignatureS(e) => {
                Error::InvalidSignatureS(e)
            }
        }
    }
}

impl MethodError for Error {
    fn encode(self) -> alloc::vec::Vec<u8> {
        self.into()
    }
}

/// State of an [`Erc20Permit`] Contract.
#[storage]
pub struct Erc20Permit<T: IEip712 + StorageType> {
    /// Contract implementing [`IEip712`] trait.
    pub(crate) eip712: T,
}

/// NOTE: Implementation of [`TopLevelStorage`] to be able use `&mut self` when
/// calling other contracts and not `&mut (impl TopLevelStorage +
/// BorrowMut<Self>)`. Should be fixed in the future by the Stylus team.
unsafe impl<T: IEip712 + StorageType> TopLevelStorage for Erc20Permit<T> {}

/// Interface for [`Erc20Permit`]
#[allow(missing_docs)]
#[public]
pub trait IErc20Permit: INonces {
    /// The error type associated to this interface.
    type Error: Into<alloc::vec::Vec<u8>>;

    // Calculated manually to include [`INonces::nonces`].
    /// Solidity interface id associated with [`IErc20Permit`] trait.
    /// Computed as a XOR of selectors for each function in the trait.
    #[must_use]
    fn interface_id() -> B32
    where
        Self: Sized,
    {
        B32::new(function_selector!("DOMAIN_SEPARATOR",))
            ^ B32::new(function_selector!("nonces", Address,))
            ^ B32::new(function_selector!(
                "permit", Address, Address, U256, U256, U8, B256, B256
            ))
    }

    /// Returns the domain separator used in the encoding of the signature for
    /// [`Self::permit`], as defined by EIP712.
    ///
    /// NOTE: The implementation should use `#[selector(name =
    /// "DOMAIN_SEPARATOR")]` to match Solidity's camelCase naming
    /// convention.
    ///
    /// # Arguments
    ///
    /// * `&self` - Read access to the contract's state.
    #[must_use]
    fn domain_separator(&self) -> B256;

    /// Sets `value` as the allowance of `spender` over `owner`'s tokens,
    /// given `owner`'s signed approval.
    ///
    /// # Arguments
    ///
    /// * `&mut self` - Write access to the contract's state. given address.
    /// * `owner` - Account that owns the tokens.
    /// * `spender` - Account that will spend the tokens.
    /// * `value` - The number of tokens being permitted to transfer by
    ///   `spender`.
    /// * `deadline` - Deadline for the permit action.
    /// * `v` - v value from the `owner`'s signature.
    /// * `r` - r value from the `owner`'s signature.
    /// * `s` - s value from the `owner`'s signature.
    ///
    /// # Errors
    ///
    /// * [`ERC2612ExpiredSignature`] - If the `deadline` param is from the
    ///   past.
    /// * [`ERC2612InvalidSigner`] - If signer is not an `owner`.
    /// * [`ecrecover::Error::InvalidSignatureS`] - If the `s` value is grater
    ///   than [`ecrecover::SIGNATURE_S_UPPER_BOUND`].
    /// * [`ecrecover::Error::InvalidSignature`] - If the recovered address is
    ///   [`Address::ZERO`].
    /// * [`erc20::Error::InvalidSpender`] - If the `spender` address is
    ///   [`Address::ZERO`].
    ///
    /// # Events
    ///
    /// * [`erc20::Approval`]
    #[allow(clippy::too_many_arguments)]
    fn permit(
        &mut self,
        owner: Address,
        spender: Address,
        value: U256,
        deadline: U256,
        v: u8,
        r: B256,
        s: B256,
    ) -> Result<(), Self::Error>;
}

impl<T: IEip712 + StorageType> Erc20Permit<T> {
    /// See [`IErc20Permit::domain_separator`].
    #[must_use]
    pub fn domain_separator(&self) -> B256 {
        self.eip712.domain_separator_v4()
    }

    /// See [`IErc20Permit::permit`].
    #[allow(clippy::too_many_arguments, clippy::missing_errors_doc)]
    pub fn permit(
        &mut self,
        owner: Address,
        spender: Address,
        value: U256,
        deadline: U256,
        v: u8,
        r: B256,
        s: B256,
        erc20: &mut Erc20,
        nonces: &mut Nonces,
    ) -> Result<(), Error> {
        if U256::from(self.vm().block_timestamp()) > deadline {
            return Err(ERC2612ExpiredSignature { deadline }.into());
        }

        let struct_hash = keccak256(StructHashTuple::abi_encode(&(
            PERMIT_TYPEHASH,
            owner,
            spender,
            value,
            nonces.use_nonce(owner),
            deadline,
        )));

        let hash: B256 = self.eip712.hash_typed_data_v4(struct_hash);

        let signer: Address = self.ec_recover(hash, v, r, s)?;

        if signer != owner {
            return Err(ERC2612InvalidSigner { signer, owner }.into());
        }

        erc20._approve(owner, spender, value, true)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use alloy_primitives::{b256, Address, B256, U256};
    use alloy_sol_types::{SolError, SolValue};
    use stylus_sdk::{
        prelude::*, stylus_core::calls::errors::MethodError, testing::TestVM,
    };

    use super::*;
    use crate::{
        test_support::{Contract, TransactionResultExt},
        token::erc20::{Approval, IErc20},
        utils::{
            cryptography::eip712::IEip712,
            nonces::{INonces, Nonces},
            precompiles::primitives::ecrecover::SIGNATURE_S_UPPER_BOUND,
        },
    };

    const SIGNED_HASH: B256 = b256!(
        "4242424242424242424242424242424242424242424242424242424242424242"
    );
    const R: B256 = b256!(
        "1111111111111111111111111111111111111111111111111111111111111111"
    );
    const S: B256 = b256!(
        "2222222222222222222222222222222222222222222222222222222222222222"
    );

    #[storage]
    struct PermitDomain;

    impl IEip712 for PermitDomain {
        const NAME: &'static str = "Permit tests";
        const VERSION: &'static str = "1";

        fn hash_typed_data_v4(&self, _struct_hash: B256) -> B256 {
            SIGNED_HASH
        }
    }

    #[storage]
    struct PermitExample {
        permit: Erc20Permit<PermitDomain>,
        erc20: Erc20,
        nonces: Nonces,
    }

    impl PermitExample {
        #[allow(clippy::too_many_arguments)]
        fn permit(
            &mut self,
            owner: Address,
            spender: Address,
            value: U256,
            deadline: U256,
            v: u8,
            r: B256,
            s: B256,
        ) -> Result<(), Error> {
            self.permit.permit(
                owner,
                spender,
                value,
                deadline,
                v,
                r,
                s,
                &mut self.erc20,
                &mut self.nonces,
            )
        }
    }

    #[public]
    #[implements(INonces, IErc20Permit<Error = Error>)]
    impl PermitExample {}

    #[public]
    impl INonces for PermitExample {
        fn nonces(&self, owner: Address) -> U256 {
            self.nonces.nonces(owner)
        }
    }

    #[public]
    impl IErc20Permit for PermitExample {
        type Error = Error;

        #[selector(name = "DOMAIN_SEPARATOR")]
        fn domain_separator(&self) -> B256 {
            self.permit.domain_separator()
        }

        fn permit(
            &mut self,
            owner: Address,
            spender: Address,
            value: U256,
            deadline: U256,
            v: u8,
            r: B256,
            s: B256,
        ) -> Result<(), Error> {
            self.permit.permit(
                owner,
                spender,
                value,
                deadline,
                v,
                r,
                s,
                &mut self.erc20,
                &mut self.nonces,
            )
        }
    }

    unsafe impl TopLevelStorage for PermitExample {}

    fn mock_recover(vm: &TestVM, v: u8, recovered: Address) {
        let mut calldata = Vec::with_capacity(128);
        calldata.extend_from_slice(SIGNED_HASH.as_slice());
        let mut v_word = [0u8; 32];
        v_word[31] = v;
        calldata.extend_from_slice(&v_word);
        calldata.extend_from_slice(R.as_slice());
        calldata.extend_from_slice(S.as_slice());
        vm.mock_static_call(
            ecrecover::ECRECOVER_ADDR,
            calldata,
            Ok(recovered.abi_encode()),
        );
    }

    #[openzeppelin_stylus_proc::vm_test]
    fn permit_accepts_valid_signature_and_updates_allowance_and_nonce(
        contract: Contract<PermitExample>,
        alice: Address,
        bob: Address,
    ) {
        let vm = contract.vm();
        vm.set_block_timestamp(100);
        mock_recover(&vm, 27, alice);
        let value = U256::from(42);
        let deadline = U256::from(101);

        contract
            .sender(alice)
            .permit(alice, bob, value, deadline, 27, R, S)
            .expect("valid permit should approve the spender");

        assert_eq!(contract.sender(alice).erc20.allowance(alice, bob), value);
        assert_eq!(contract.sender(alice).nonces.nonces(alice), U256::ONE);
        assert_ne!(
            contract.sender(alice).permit.domain_separator(),
            B256::ZERO
        );
        contract.assert_emitted(&Approval {
            owner: alice,
            spender: bob,
            value,
        });
    }

    #[openzeppelin_stylus_proc::vm_test]
    fn public_permit_traits_forward_to_storage(
        contract: Contract<PermitExample>,
        alice: Address,
        bob: Address,
    ) {
        let vm = contract.vm();
        vm.set_block_timestamp(100);
        mock_recover(&vm, 27, alice);

        let value = U256::from(42);
        let mut call = contract.sender(alice);
        assert_eq!(INonces::nonces(&*call, alice), U256::ZERO);
        assert_eq!(
            IErc20Permit::domain_separator(&*call),
            call.permit.domain_separator()
        );

        IErc20Permit::permit(
            &mut *call,
            alice,
            bob,
            value,
            U256::from(101),
            27,
            R,
            S,
        )
        .expect("the public trait should forward a valid permit");

        assert_eq!(INonces::nonces(&*call, alice), U256::ONE);
        assert_eq!(call.erc20.allowance(alice, bob), value);
    }

    #[test]
    fn interface_id_includes_domain_separator_nonces_and_permit() {
        let actual = <PermitExample as IErc20Permit>::interface_id();
        let expected: B32 = 0x9d8ff7da_u32.into();
        assert_eq!(actual, expected);
    }

    #[test]
    fn error_encodes_expired_signature_abi() {
        let deadline = U256::from(99);
        let encoded = <Error as MethodError>::encode(
            ERC2612ExpiredSignature { deadline }.into(),
        );
        let expected = ERC2612ExpiredSignature { deadline }.abi_encode();
        assert_eq!(encoded, expected);
    }

    #[openzeppelin_stylus_proc::vm_test]
    fn permit_rejects_expired_deadline_before_recovering_signature(
        contract: Contract<PermitExample>,
        alice: Address,
        bob: Address,
    ) {
        contract.vm().set_block_timestamp(100);

        let err = contract
            .sender(alice)
            .permit(alice, bob, U256::from(1), U256::from(99), 27, R, S)
            .expect_error("expired permit must fail before signature recovery");

        assert!(matches!(
            err,
            Error::ExpiredSignature(ERC2612ExpiredSignature { deadline })
                if deadline == U256::from(99)
        ));
        assert_eq!(contract.sender(alice).nonces.nonces(alice), U256::ZERO);
    }

    #[openzeppelin_stylus_proc::vm_test]
    fn permit_rejects_signature_from_another_account(
        contract: Contract<PermitExample>,
        alice: Address,
        bob: Address,
        charlie: Address,
    ) {
        contract.vm().set_block_timestamp(100);
        mock_recover(&contract.vm(), 27, charlie);

        let err = contract
            .sender(alice)
            .permit(alice, bob, U256::from(1), U256::from(100), 27, R, S)
            .expect_error("permit must recover the owner address");

        assert!(matches!(
            err,
            Error::InvalidSigner(ERC2612InvalidSigner { signer, owner })
                if signer == charlie && owner == alice
        ));
        assert_eq!(contract.sender(alice).nonces.nonces(alice), U256::ZERO);
    }

    #[openzeppelin_stylus_proc::vm_test]
    fn permit_rejects_malleable_signature_s(
        contract: Contract<PermitExample>,
        alice: Address,
        bob: Address,
    ) {
        contract.vm().set_block_timestamp(100);
        let high_s = B256::from_slice(
            &(SIGNATURE_S_UPPER_BOUND + U256::ONE).to_be_bytes::<32>(),
        );

        let err = contract
            .sender(alice)
            .permit(alice, bob, U256::from(1), U256::from(100), 27, R, high_s)
            .expect_error("malleable signature s must be rejected");

        assert!(matches!(
            err,
            Error::InvalidSignatureS(ECDSAInvalidSignatureS { s }) if s == high_s
        ));
        assert_eq!(contract.sender(alice).nonces.nonces(alice), U256::ZERO);
    }

    #[openzeppelin_stylus_proc::vm_test]
    fn permit_rejects_zero_recovered_signer(
        contract: Contract<PermitExample>,
        alice: Address,
        bob: Address,
    ) {
        contract.vm().set_block_timestamp(100);
        mock_recover(&contract.vm(), 27, Address::ZERO);

        let err = contract
            .sender(alice)
            .permit(alice, bob, U256::from(1), U256::from(100), 27, R, S)
            .expect_error("a signature recovering the zero address is invalid");

        assert!(matches!(err, Error::InvalidSignature(_)));
        assert_eq!(contract.sender(alice).nonces.nonces(alice), U256::ZERO);
    }

    #[openzeppelin_stylus_proc::vm_test]
    fn permit_propagates_invalid_spender_without_consuming_nonce(
        contract: Contract<PermitExample>,
        alice: Address,
    ) {
        contract.vm().set_block_timestamp(100);
        mock_recover(&contract.vm(), 27, alice);

        let err = contract
            .sender(alice)
            .permit(
                alice,
                Address::ZERO,
                U256::from(1),
                U256::from(100),
                27,
                R,
                S,
            )
            .expect_error("zero spender must be rejected by ERC-20 approval");

        assert!(matches!(err, Error::InvalidSpender(_)));
        assert_eq!(contract.sender(alice).nonces.nonces(alice), U256::ZERO);
    }
}
