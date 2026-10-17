extern crate alloc;

use alloy_primitives::{uint, Address, U256, U8};
use alloy_sol_types::{SolCall, SolType, SolValue};
use openzeppelin_stylus::token::erc20::{
    extensions::{erc4626::Erc4626, wrapper::Erc20Wrapper},
    interface::Erc20Interface,
    Erc20, IErc20,
};
use stylus_sdk::{prelude::*, stylus_core::calls::Call, testing::TestVM};

alloy_sol_types::sol! {
    interface FeeOnTransferToken {
        function transferFrom(address from, address to, uint256 value) external returns (bool);
        function balanceOf(address account) external view returns (uint256);
    }
}

#[storage]
struct WrapperContract {
    wrapper: Erc20Wrapper,
    erc20: Erc20,
}

unsafe impl TopLevelStorage for WrapperContract {}

#[storage]
struct VaultContract {
    vault: Erc4626,
    erc20: Erc20,
}

unsafe impl TopLevelStorage for VaultContract {}

fn mock_balance(vm: &TestVM, token: Address, account: Address, balance: U256) {
    let call = FeeOnTransferToken::balanceOfCall { account };
    vm.mock_static_call(
        token,
        call.abi_encode(),
        Ok(<alloy_sol_types::sol_data::Uint<256> as SolType>::abi_encode(
            &balance,
        )),
    );
}

#[test]
fn fee_on_transfer_deposit_mints_nominal_wrapper_shares() {
    let vm = TestVM::new();
    let wrapper_address = Address::with_last_byte(1);
    let underlying = Address::with_last_byte(2);
    let alice = Address::with_last_byte(3);
    let requested = uint!(100_U256);
    let received = uint!(90_U256);

    vm.set_contract_address(wrapper_address);
    vm.set_sender(alice);
    vm.set_code(underlying, vec![0x00]);

    let transfer = FeeOnTransferToken::transferFromCall {
        from: alice,
        to: wrapper_address,
        value: requested,
    };
    vm.mock_call(
        underlying,
        transfer.abi_encode(),
        U256::ZERO,
        Ok(true.abi_encode()),
    );

    let balance =
        FeeOnTransferToken::balanceOfCall { account: wrapper_address };
    vm.mock_static_call(
        underlying,
        balance.abi_encode(),
        Ok(<alloy_sol_types::sol_data::Uint<256> as SolType>::abi_encode(
            &received,
        )),
    );

    let mut contract = WrapperContract::from(&vm);
    contract.wrapper.constructor(underlying).expect("should construct");
    contract
        .wrapper
        .deposit_for(alice, requested, &mut contract.erc20)
        .expect("transfer should succeed");

    let assets = Erc20Interface::new(underlying)
        .balance_of(&vm, Call::new(), wrapper_address)
        .expect("balanceOf should succeed");
    let shares = contract.erc20.total_supply();

    assert_eq!(shares, requested);
    assert_eq!(assets, received);
    assert!(shares > assets, "fee-on-transfer deposits are unsupported");
}

#[test]
fn fee_on_transfer_vault_deposit_mints_nominal_shares() {
    let vm = TestVM::new();
    let vault_address = Address::with_last_byte(4);
    let underlying = Address::with_last_byte(5);
    let alice = Address::with_last_byte(6);
    let requested = uint!(100_U256);
    let received = uint!(90_U256);

    vm.set_contract_address(vault_address);
    vm.set_sender(alice);
    vm.set_code(underlying, vec![0x00]);

    let mut contract = VaultContract::from(&vm);
    contract.vault.constructor(underlying, U8::ZERO);
    let transfer = FeeOnTransferToken::transferFromCall {
        from: alice,
        to: vault_address,
        value: requested,
    };
    vm.mock_call(
        underlying,
        transfer.abi_encode(),
        U256::ZERO,
        Ok(true.abi_encode()),
    );
    contract
        .vault
        ._deposit(alice, alice, requested, requested, &mut contract.erc20)
        .expect("transfer should succeed");

    mock_balance(&vm, underlying, vault_address, received);
    let assets =
        contract.vault.total_assets().expect("balanceOf should succeed");
    let shares = contract.erc20.total_supply();

    assert_eq!(shares, requested);
    assert_eq!(assets, received);
    assert!(shares > assets, "fee-on-transfer assets are unsupported");
}
