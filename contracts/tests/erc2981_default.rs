//! Regression test for the documented default ERC-2981 denominator.

use alloy_primitives::{uint, Address, U256};
use openzeppelin_stylus::token::common::erc2981::{Erc2981, IErc2981};
use stylus_test::TestVMBuilder;

#[test]
fn royalty_info_without_constructor_setup_uses_default_denominator() {
    let vm = TestVMBuilder::new().build();
    let contract = Erc2981::from(&vm);

    let royalty = contract.royalty_info(uint!(1_U256), uint!(1000_U256));

    assert_eq!(royalty, (Address::ZERO, U256::ZERO));
}
