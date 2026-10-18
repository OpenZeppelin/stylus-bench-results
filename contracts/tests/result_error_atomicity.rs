//! Regression coverage for library methods that return errors after state changes.
//!
//! A contract can catch a library `Result::Err` and return success from its
//! public entry point. In that case the Stylus call does not revert those
//! earlier storage writes, so library methods must validate before mutating.

use alloy_primitives::{Address, U256, address, uint};
use openzeppelin_stylus::token::{
    erc20::{Erc20, Error as Erc20Error, IErc20},
    erc721::{Erc721, Error as Erc721Error, IErc721},
    erc1155::{Erc1155, Error as Erc1155Error, IErc1155},
};
use stylus_sdk::abi::Bytes;
use stylus_test::TestVMBuilder;

const ALICE: Address = address!("0000000000000000000000000000000000000001");
const BOB: Address = address!("0000000000000000000000000000000000000002");
const SPENDER: Address = address!("0000000000000000000000000000000000000003");
const OTHER: Address = address!("0000000000000000000000000000000000000004");
const CONTRACT: Address = address!("0000000000000000000000000000000000000100");

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
