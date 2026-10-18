//! Regression test for full precision linear vesting calculations.

use alloy_primitives::{Address, U256, U64};
use openzeppelin_stylus::finance::vesting_wallet::{
    IVestingWallet, VestingWallet,
};
use stylus_sdk::testing::TestVM;

#[test]
fn vested_amount_avoids_intermediate_overflow() {
    let vm = TestVM::default();
    let wallet_address = Address::from([9; 20]);
    vm.set_contract_address(wallet_address);
    let mut vesting = VestingWallet::from(&vm);
    let beneficiary = Address::from([1; 20]);
    vesting
        .constructor(beneficiary, U64::ZERO, U64::MAX)
        .expect("constructor should succeed");

    vm.set_balance(wallet_address, U256::MAX);
    let elapsed = U64::MAX - U64::from(1);
    let vested = vesting.vested_amount_eth(elapsed.to::<u64>());

    // floor(MAX * (duration - 1) / duration) = MAX - ceil(MAX / duration).
    let duration = U256::from(U64::MAX);
    let quotient = U256::MAX / duration;
    let remainder = U256::MAX % duration;
    let ceil_max_over_duration = quotient + U256::from(!remainder.is_zero());
    assert_eq!(U256::MAX - ceil_max_over_duration, vested);
}
