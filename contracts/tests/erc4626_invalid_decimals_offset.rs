//! Regression `PoCs` for ERC-4626 decimal offsets that make a vault unusable.

use alloy_primitives::{Address, U8};
use openzeppelin_stylus::token::erc20::extensions::Erc4626;
use stylus_sdk::testing::TestVM;

fn vault_with_asset_decimals(decimals: u8) -> (TestVM, Erc4626) {
    let vm = TestVM::new();
    let asset = Address::from([0x11; 20]);
    let mut encoded_decimals = [0_u8; 32];
    encoded_decimals[31] = decimals;
    vm.mock_static_call(
        asset,
        [0x31, 0x3c, 0xe5, 0x67].to_vec(),
        Ok(encoded_decimals.to_vec()),
    );
    let vault = Erc4626::from(&vm);
    (vm, vault)
}

#[test]
fn constructor_does_not_store_offset_that_overflows_pow10() {
    let (_vm, mut vault) = vault_with_asset_decimals(18);
    let asset = Address::from([0x11; 20]);

    let result = vault.constructor(asset, U8::from(78));

    assert!(result.is_err());
    assert_eq!(U8::ZERO, vault._decimals_offset());
}

#[test]
fn constructor_does_not_store_offset_that_overflows_share_decimals() {
    let (_vm, mut vault) = vault_with_asset_decimals(u8::MAX);
    let asset = Address::from([0x11; 20]);

    let result = vault.constructor(asset, U8::from(1));

    assert!(result.is_err());
    assert_eq!(U8::ZERO, vault._decimals_offset());
}
