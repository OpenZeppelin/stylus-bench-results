//! SafeERC20 must reject noncanonical ABI boolean return data.

use alloy_primitives::{Address, U256};
use alloy_sol_types::{sol, SolCall, SolValue};
use openzeppelin_stylus::token::erc20::utils::safe_erc20::{
    Error, ISafeErc20, SafeErc20,
};
use stylus_sdk::testing::TestVM;

sol! {
    function transfer(address to, uint256 value) external returns (bool);
}

#[test]
fn safe_transfer_rejects_noncanonical_true_return() {
    let vm = TestVM::new();
    let contract = Address::from([1; 20]);
    let token = Address::from([2; 20]);
    let recipient = Address::from([3; 20]);
    vm.set_contract_address(contract);
    vm.set_code(token, vec![0x00]);

    let calldata = transferCall {
        to: recipient,
        value: U256::from(1),
    }
    .abi_encode();
    vm.mock_call(
        token,
        calldata,
        U256::ZERO,
        Ok(U256::from(2).abi_encode()),
    );

    let mut safe_erc20 = SafeErc20::from(&vm);
    let result = safe_erc20.safe_transfer(token, recipient, U256::from(1));

    assert!(matches!(result, Err(Error::SafeErc20FailedOperation(_))));
}
