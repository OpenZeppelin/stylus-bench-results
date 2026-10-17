//! Regression tests for errors returned after partial state changes.

use alloy_primitives::{Address, U256};
use openzeppelin_stylus::token::{
    erc1155::{Erc1155, IErc1155},
    erc721::{Erc721, IErc721},
};
use stylus_sdk::testing::TestVM;

#[test]
fn duplicate_erc721_mint_error_does_not_change_owner_or_emit_transfer() {
    let vm = TestVM::default();
    let mut token = Erc721::from(&vm);
    let alice = Address::from([1; 20]);
    let bob = Address::from([2; 20]);
    let id = U256::from(1);

    token._mint(alice, id).unwrap();
    let logs_before = vm.get_emitted_logs();

    assert!(token._mint(bob, id).is_err());

    assert_eq!(token.owner_of(id).unwrap(), alice);
    assert_eq!(vm.get_emitted_logs(), logs_before);
}

#[test]
fn incorrect_erc721_from_error_does_not_transfer_or_emit_transfer() {
    let vm = TestVM::default();
    let mut token = Erc721::from(&vm);
    let alice = Address::from([1; 20]);
    let bob = Address::from([2; 20]);
    let charlie = Address::from([3; 20]);
    let id = U256::from(1);

    token._mint(alice, id).unwrap();
    vm.set_sender(alice);
    let logs_before = vm.get_emitted_logs();

    assert!(token.transfer_from(bob, charlie, id).is_err());

    assert_eq!(token.owner_of(id).unwrap(), alice);
    assert_eq!(vm.get_emitted_logs(), logs_before);
}

#[test]
fn failed_erc1155_batch_burn_does_not_partially_burn_or_emit_transfer() {
    let vm = TestVM::default();
    let mut token = Erc1155::from(&vm);
    let alice = Address::from([1; 20]);
    let first_id = U256::from(1);
    let second_id = U256::from(2);

    token
        ._mint_batch(
            alice,
            vec![first_id, second_id],
            vec![U256::from(1), U256::from(1)],
            &Vec::new().into(),
        )
        .unwrap();
    let logs_before = vm.get_emitted_logs();

    assert!(token
        ._burn_batch(
            alice,
            vec![first_id, second_id],
            vec![U256::from(1), U256::from(2)],
        )
        .is_err());

    assert_eq!(token.balance_of(alice, first_id), U256::from(1));
    assert_eq!(token.balance_of(alice, second_id), U256::from(1));
    assert_eq!(vm.get_emitted_logs(), logs_before);
}
