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
#[public]
// The `#[public]` macro adds an undocumented helper method to the trait.
#[allow(missing_docs)]
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
    use alloy_primitives::{b256, uint};
    use stylus_sdk::testing::TestVM;

    use super::*;
    use crate::{
        test_utils::{ALICE, BOB},
        token::erc20::{tests::assert_emitted, IErc20},
        utils::precompiles::primitives::ecrecover::{
            EcRecoverData, ECRECOVER_ADDR,
        },
    };

    const V: u8 = 28;
    const R: B256 = b256!(
        "65e72b1cf8e189569963750e10ccb88fe89389daeeb8b735277d59cd6885ee82"
    );
    const S: B256 = b256!(
        "3eb5a6982b540f185703492dab77b863a88ce01f27e21ade8b2879c10fc9e653"
    );

    #[storage]
    struct Eip712;

    impl IEip712 for Eip712 {
        const NAME: &'static str = "ERC-20 Permit Test";
        const VERSION: &'static str = "1";
    }

    #[storage]
    struct Erc20PermitTestExample {
        erc20: Erc20,
        nonces: Nonces,
        erc20_permit: Erc20Permit<Eip712>,
    }

    /// Mocks `ecRecover` to recover `signer` from the signature [`V`], [`R`]
    /// and [`S`] of the permit of `value` from [`ALICE`] to [`BOB`] with
    /// `deadline`, as the first permit of [`ALICE`].
    ///
    /// `TestVM` doesn't run precompiles.
    fn mock_ec_recover(
        vm: &TestVM,
        contract: &Erc20PermitTestExample,
        value: U256,
        deadline: U256,
        signer: Address,
    ) {
        let struct_hash = keccak256(StructHashTuple::abi_encode(&(
            PERMIT_TYPEHASH,
            ALICE,
            BOB,
            value,
            U256::ZERO,
            deadline,
        )));
        let hash = contract.erc20_permit.eip712.hash_typed_data_v4(struct_hash);
        vm.mock_static_call(
            ECRECOVER_ADDR,
            EcRecoverData::abi_encode(&EcRecoverData {
                hash,
                v: V,
                r: R,
                s: S,
            }),
            Ok(signer.into_word().to_vec()),
        );
    }

    #[test]
    fn permit_works() {
        let vm = TestVM::default();
        let mut contract = Erc20PermitTestExample::from(&vm);

        let value = uint!(1000_U256);
        let deadline = uint!(2000_U256);
        vm.set_block_timestamp(1000);
        mock_ec_recover(&vm, &contract, value, deadline, ALICE);

        contract
            .erc20_permit
            .permit(
                ALICE,
                BOB,
                value,
                deadline,
                V,
                R,
                S,
                &mut contract.erc20,
                &mut contract.nonces,
            )
            .expect("should permit");

        assert_eq!(contract.erc20.allowance(ALICE, BOB), value);
        assert_eq!(contract.nonces.nonces(ALICE), uint!(1_U256));
        assert_emitted(
            &vm,
            &erc20::Approval { owner: ALICE, spender: BOB, value },
        );
    }

    #[test]
    fn permit_reverts_when_deadline_expired() {
        let vm = TestVM::default();
        let mut contract = Erc20PermitTestExample::from(&vm);

        let deadline = uint!(1000_U256);
        vm.set_block_timestamp(1001);

        let err = contract
            .erc20_permit
            .permit(
                ALICE,
                BOB,
                uint!(1000_U256),
                deadline,
                V,
                R,
                S,
                &mut contract.erc20,
                &mut contract.nonces,
            )
            .expect_err("should return Error::ExpiredSignature");

        assert!(matches!(
            err,
            Error::ExpiredSignature(ERC2612ExpiredSignature { deadline: d })
                if d == deadline
        ));
    }

    #[test]
    fn permit_reverts_when_signer_is_not_owner() {
        let vm = TestVM::default();
        let mut contract = Erc20PermitTestExample::from(&vm);

        let value = uint!(1000_U256);
        let deadline = uint!(2000_U256);
        vm.set_block_timestamp(1000);
        mock_ec_recover(&vm, &contract, value, deadline, BOB);

        let err = contract
            .erc20_permit
            .permit(
                ALICE,
                BOB,
                value,
                deadline,
                V,
                R,
                S,
                &mut contract.erc20,
                &mut contract.nonces,
            )
            .expect_err("should return Error::InvalidSigner");

        assert!(matches!(
            err,
            Error::InvalidSigner(ERC2612InvalidSigner { signer, owner })
                if signer == BOB && owner == ALICE
        ));
        assert_eq!(contract.erc20.allowance(ALICE, BOB), U256::ZERO);
    }
}
