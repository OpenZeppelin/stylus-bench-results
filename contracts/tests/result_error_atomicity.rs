//! Regression coverage for library methods that return errors after state changes.
//!
//! A contract can catch a library `Result::Err` and return success from its
//! public entry point. In that case the Stylus call does not revert those
//! earlier storage writes, so library methods must validate before mutating.

extern crate alloc;

use alloy_primitives::{Address, B256, U256, address, keccak256, uint};
use alloy_sol_types::{SolType, SolValue, sol};
use openzeppelin_stylus::token::{
    erc20::extensions::{Erc20Permit, permit},
    erc20::{Erc20, Error as Erc20Error, IErc20},
    erc721::{Erc721, Error as Erc721Error, IErc721},
    erc1155::{Erc1155, Error as Erc1155Error, IErc1155},
};
use openzeppelin_stylus::utils::{
    cryptography::eip712::{IEip712, to_typed_data_hash},
    nonces::{INonces, Nonces},
    precompiles::primitives::ecrecover::{ECRECOVER_ADDR, EcRecoverData},
};
use stylus_sdk::{abi::Bytes, prelude::*};
use stylus_test::{TestVM, TestVMBuilder};

const ALICE: Address = address!("0000000000000000000000000000000000000001");
const BOB: Address = address!("0000000000000000000000000000000000000002");
const SPENDER: Address = address!("0000000000000000000000000000000000000003");
const OTHER: Address = address!("0000000000000000000000000000000000000004");
const CONTRACT: Address = address!("0000000000000000000000000000000000000100");
const PERMIT_TYPEHASH: [u8; 32] = keccak_const::Keccak256::new()
    .update(b"Permit(address owner,address spender,uint256 value,uint256 nonce,uint256 deadline)")
    .finalize();

type PermitStructHashTuple =
    sol! { tuple(bytes32, address, address, uint256, uint256, uint256) };

#[storage]
struct TestEip712;

impl IEip712 for TestEip712 {
    const NAME: &'static str = "F04 Permit Test";
    const VERSION: &'static str = "1";
}

#[storage]
struct TestPermitContract {
    erc20: Erc20,
    nonces: Nonces,
    permit: Erc20Permit<TestEip712>,
}

fn mock_recovered_signer(
    vm: &TestVM,
    domain_separator: B256,
    owner: Address,
    spender: Address,
    value: U256,
    deadline: U256,
    signer: Address,
) -> (u8, B256, B256) {
    let nonce = U256::ZERO;
    let struct_hash = keccak256(PermitStructHashTuple::abi_encode(&(
        PERMIT_TYPEHASH,
        owner,
        spender,
        value,
        nonce,
        deadline,
    )));
    let hash = to_typed_data_hash(&domain_separator, &struct_hash);
    let v = 27;
    let r = B256::from([1_u8; 32]);
    let s = B256::from([2_u8; 32]);
    let input = EcRecoverData { hash, v, r, s }.abi_encode();
    let mut output = [0_u8; 32];
    output[12..].copy_from_slice(signer.as_slice());
    vm.mock_static_call(ECRECOVER_ADDR, input, Ok(output.to_vec()));
    (v, r, s)
}

fn consumer_catches<T, E>(_result: Result<T, E>) -> Result<(), ()> {
    Ok(())
}

#[test]
fn erc721_duplicate_mint_error_does_not_transfer_existing_token() {
    let vm = TestVMBuilder::new().contract_address(CONTRACT).build();
    let mut token = Erc721::from(&vm);
    let token_id = uint!(17_U256);

    token._mint(ALICE, token_id).unwrap();
    let result = token._mint(BOB, token_id);
    assert!(matches!(result, Err(Erc721Error::InvalidSender(_))));

    consumer_catches(result).unwrap();
    assert_eq!(token._owner_of(token_id), ALICE);
}

#[test]
fn erc721_incorrect_owner_internal_transfer_error_keeps_owner() {
    let vm = TestVMBuilder::new().contract_address(CONTRACT).build();
    let mut token = Erc721::from(&vm);
    let token_id = uint!(18_U256);

    token._mint(ALICE, token_id).unwrap();
    let result = token._transfer(OTHER, BOB, token_id);
    assert!(matches!(result, Err(Erc721Error::IncorrectOwner(_))));

    consumer_catches(result).unwrap();
    assert_eq!(token._owner_of(token_id), ALICE);
}

#[test]
fn erc721_public_transfer_from_incorrect_owner_error_keeps_owner() {
    let vm = TestVMBuilder::new().contract_address(CONTRACT).build();
    let mut token = Erc721::from(&vm);
    let token_id = uint!(19_U256);

    token._mint(ALICE, token_id).unwrap();
    vm.set_sender(ALICE);
    let result = IErc721::transfer_from(&mut token, OTHER, BOB, token_id);
    assert!(matches!(result, Err(Erc721Error::IncorrectOwner(_))));

    consumer_catches(result).unwrap();
    assert_eq!(token._owner_of(token_id), ALICE);
}

#[test]
fn erc721_burning_nonexistent_token_error_emits_no_transfer() {
    let vm = TestVMBuilder::new().contract_address(CONTRACT).build();
    let mut token = Erc721::from(&vm);
    let result = token._burn(uint!(20_U256));
    assert!(matches!(result, Err(Erc721Error::NonexistentToken(_))));

    consumer_catches(result).unwrap();
    assert!(vm.get_emitted_logs().is_empty());
}

#[test]
fn erc1155_batch_later_insufficient_balance_error_keeps_earlier_balances() {
    let vm = TestVMBuilder::new().contract_address(CONTRACT).build();
    let mut token = Erc1155::from(&vm);
    let first_id = uint!(1_U256);
    let second_id = uint!(2_U256);

    let data = Bytes::default();
    token._mint(ALICE, first_id, uint!(5_U256), &data).unwrap();
    token._mint(ALICE, second_id, uint!(1_U256), &data).unwrap();
    vm.set_sender(ALICE);

    let result = IErc1155::safe_batch_transfer_from(
        &mut token,
        ALICE,
        BOB,
        vec![first_id, second_id],
        vec![uint!(2_U256), uint!(2_U256)],
        Bytes::default(),
    );
    assert!(matches!(result, Err(Erc1155Error::InsufficientBalance(_))));

    consumer_catches(result).unwrap();
    assert_eq!(token.balance_of(ALICE, first_id), uint!(5_U256));
    assert_eq!(token.balance_of(BOB, first_id), U256::ZERO);
}

#[test]
fn erc20_transfer_from_insufficient_balance_error_keeps_allowance() {
    let vm = TestVMBuilder::new().contract_address(CONTRACT).build();
    let mut token = Erc20::from(&vm);
    let amount = uint!(2_U256);

    vm.set_sender(ALICE);
    IErc20::approve(&mut token, SPENDER, amount).unwrap();
    vm.set_sender(SPENDER);
    let result = IErc20::transfer_from(&mut token, ALICE, BOB, amount);
    assert!(matches!(result, Err(Erc20Error::InsufficientBalance(_))));

    consumer_catches(result).unwrap();
    assert_eq!(token.allowance(ALICE, SPENDER), amount);
}

#[test]
fn erc20_permit_invalid_signer_error_keeps_nonce() {
    let vm =
        TestVMBuilder::new().chain_id(42161).contract_address(CONTRACT).build();
    let mut contract = TestPermitContract::from(&vm);
    let value = uint!(2_U256);
    let deadline = U256::MAX;
    let (v, r, s) = mock_recovered_signer(
        &vm,
        contract.permit.domain_separator(),
        ALICE,
        BOB,
        value,
        deadline,
        OTHER,
    );

    let result = contract.permit.permit(
        ALICE,
        BOB,
        value,
        deadline,
        v,
        r,
        s,
        &mut contract.erc20,
        &mut contract.nonces,
    );
    assert!(matches!(result, Err(permit::Error::InvalidSigner(_))));

    consumer_catches(result).unwrap();
    assert_eq!(contract.nonces.nonces(ALICE), U256::ZERO);
}

#[test]
fn erc20_permit_zero_spender_error_keeps_nonce() {
    let vm =
        TestVMBuilder::new().chain_id(42161).contract_address(CONTRACT).build();
    let mut contract = TestPermitContract::from(&vm);
    let value = uint!(2_U256);
    let deadline = U256::MAX;
    let (v, r, s) = mock_recovered_signer(
        &vm,
        contract.permit.domain_separator(),
        ALICE,
        Address::ZERO,
        value,
        deadline,
        ALICE,
    );

    let result = contract.permit.permit(
        ALICE,
        Address::ZERO,
        value,
        deadline,
        v,
        r,
        s,
        &mut contract.erc20,
        &mut contract.nonces,
    );
    assert!(matches!(result, Err(permit::Error::InvalidSpender(_))));

    consumer_catches(result).unwrap();
    assert_eq!(contract.nonces.nonces(ALICE), U256::ZERO);
}
