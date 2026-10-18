//! Regression coverage for the ERC721Consecutive constructor-only mint path.

use alloy_primitives::{address, aliases::U96, uint, Address, U256};
use alloy_sol_types::SolEvent;
use openzeppelin_stylus::token::erc721::{
    extensions::{
        consecutive::{self, Error},
        Erc721Consecutive,
    },
    IErc721,
};
use stylus_test::TestVMBuilder;

const CONTRACT_ADDRESS: Address =
    address!("0000000000000000000000000000000000000001");
const RECIPIENT: Address = address!("0000000000000000000000000000000000000002");
const MAX_BATCH_SIZE: U96 = uint!(5000_U96);

fn new_contract() -> (stylus_test::TestVM, Erc721Consecutive) {
    let vm = TestVMBuilder::new().contract_address(CONTRACT_ADDRESS).build();
    let mut contract = Erc721Consecutive::from(&vm);
    contract.max_batch_size.set(MAX_BATCH_SIZE);
    (vm, contract)
}

#[test]
fn rejects_consecutive_mint_without_construction_phase() {
    let (_vm, mut contract) = new_contract();
    let err = contract._mint_consecutive(RECIPIENT, uint!(1_U96)).expect_err(
        "consecutive mint outside the constructor phase should fail",
    );

    assert!(matches!(err, Error::ForbiddenBatchMint(_)));
}

#[test]
fn allows_consecutive_mint_during_construction() {
    let (vm, mut contract) = new_contract();
    contract
        ._mint_consecutive_batches([(RECIPIENT, uint!(1_U96))])
        .expect("constructor consecutive batch should succeed");
    assert_eq!(
        contract
            .erc721
            .balance_of(RECIPIENT)
            .expect("recipient balance query should succeed"),
        uint!(1_U256)
    );

    let expected = consecutive::ConsecutiveTransfer {
        from_token_id: U256::ZERO,
        to_token_id: U256::ZERO,
        from_address: Address::ZERO,
        to_address: RECIPIENT,
    }
    .encode_log_data();
    assert!(vm.get_emitted_logs().iter().any(|(topics, data)| {
        topics.as_slice() == expected.topics()
            && data.as_slice() == expected.data.as_ref()
    }));
}

#[test]
fn completed_construction_phase_cannot_reopen() {
    let (_vm, mut contract) = new_contract();
    contract
        ._mint_consecutive_batches([(RECIPIENT, uint!(1_U96))])
        .expect("constructor consecutive batch should succeed");

    let error = contract
        ._mint_consecutive_batches([(RECIPIENT, uint!(1_U96))])
        .expect_err("completed construction phase must stay closed");
    assert!(matches!(error, Error::ForbiddenBatchMint(_)));
}
