//! Regression coverage for the ERC721Consecutive constructor-only mint path.

use alloy_primitives::{address, aliases::U96, uint, Address, U256};
use alloy_sol_types::SolEvent;
use openzeppelin_stylus::token::erc721::extensions::{
    consecutive::{self, Error},
    Erc721Consecutive,
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
fn rejects_consecutive_mint_after_deployment() {
    let (vm, mut contract) = new_contract();
    vm.set_code(CONTRACT_ADDRESS, vec![0x00]);

    let err = contract
        ._mint_consecutive(RECIPIENT, uint!(1_U96))
        .expect_err("runtime consecutive mint should be rejected");

    assert!(matches!(err, Error::ForbiddenBatchMint(_)));
}

#[test]
fn allows_consecutive_mint_during_construction() {
    let (vm, mut contract) = new_contract();

    let first_token_id = contract
        ._mint_consecutive(RECIPIENT, uint!(1_U96))
        .expect("constructor consecutive mint should succeed");
    assert_eq!(first_token_id, U96::ZERO);

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
