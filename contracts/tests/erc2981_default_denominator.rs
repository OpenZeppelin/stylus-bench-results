//! Regression test for the documented ERC2981 default denominator.

use alloy_primitives::{aliases::U96, Address, U256};
use openzeppelin_stylus::token::common::erc2981::{Erc2981, IErc2981};
use stylus_sdk::testing::TestVM;

#[test]
fn fresh_instance_uses_basis_point_denominator() {
    let vm = TestVM::default();
    let contract = Erc2981::from(&vm);

    assert_eq!(U96::from(10000u16), contract._fee_denominator());
    assert_eq!(
        (Address::ZERO, U256::ZERO),
        contract.royalty_info(U256::ZERO, U256::MAX)
    );
}
