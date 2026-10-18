//! Test that ERC-2981 royalty calculation handles a representable quotient.

use alloy_primitives::{uint, Address, U256};
use openzeppelin_stylus::token::common::erc2981::{Erc2981, IErc2981};
use stylus_sdk::testing::TestVM;

#[test]
fn royalty_info_handles_large_sale_prices_without_intermediate_overflow() {
    let vm = TestVM::default();
    let mut contract = Erc2981::from(&vm);
    let receiver = Address::from([1; 20]);

    contract.fee_denominator.set(uint!(10000_U96));
    contract
        ._set_default_royalty(receiver, uint!(9000_U96))
        .expect("should set default royalty");

    let (actual_receiver, amount) =
        contract.royalty_info(U256::ZERO, U256::MAX);

    // For a 90% royalty, floor(MAX * 9 / 10) is MAX - ceil(MAX / 10).
    assert_eq!(receiver, actual_receiver);
    assert_eq!(
        U256::MAX - (U256::MAX / uint!(10_U256) + uint!(1_U256)),
        amount
    );
}
