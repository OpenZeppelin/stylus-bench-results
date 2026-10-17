extern crate alloc;

use alloy_primitives::{Address, U256};
use alloy_sol_types::{sol, SolCall};
use openzeppelin_stylus::token::erc20::{
    extensions::{
        capped::Capped,
        flash_mint::{Erc20FlashMint, BORROWER_CALLBACK_VALUE},
    },
    Erc20, IErc20,
};
use stylus_sdk::{abi::Bytes, prelude::*, testing::TestVM};

sol! {
    interface FlashBorrower {
        function onFlashLoan(
            address initiator,
            address token,
            uint256 amount,
            uint256 fee,
            bytes data
        ) external returns (bytes32);
    }
}

#[storage]
struct CappedFlashMintToken {
    capped: Capped,
    flash_mint: Erc20FlashMint,
    erc20: Erc20,
}

unsafe impl TopLevelStorage for CappedFlashMintToken {}

#[test]
fn flash_loan_must_not_exceed_the_composed_supply_cap() {
    let vm = TestVM::default();
    let token = Address::from([0x10; 20]);
    let borrower = Address::from([0x20; 20]);
    let alice = Address::from([0x30; 20]);
    let cap = U256::from(100);
    let initial_supply = U256::from(90);
    let loan = U256::from(20);
    vm.set_contract_address(token);
    vm.set_code(borrower, vec![1]);

    let mut contract = CappedFlashMintToken::from(&vm);
    contract.capped.constructor(cap).expect("valid cap");
    vm.set_sender(alice);
    contract.erc20._mint(alice, initial_supply).expect("initial mint");

    vm.set_sender(borrower);
    contract.erc20.approve(token, loan).expect("repayment approval");

    let callback = FlashBorrower::onFlashLoanCall {
        initiator: alice,
        token,
        amount: loan,
        fee: U256::ZERO,
        data: Bytes::new(),
    }
    .abi_encode();
    vm.mock_call(
        borrower,
        callback,
        U256::ZERO,
        Ok(BORROWER_CALLBACK_VALUE.to_vec()),
    );

    vm.set_sender(alice);
    let result = contract.flash_mint.flash_loan(
        borrower,
        token,
        loan,
        &Bytes::new(),
        &mut contract.erc20,
    );

    assert!(
        result.is_err(),
        "a 20-token flash mint must fail when only 10 tokens remain under the cap"
    );
    assert_eq!(contract.erc20.total_supply(), initial_supply);
}
