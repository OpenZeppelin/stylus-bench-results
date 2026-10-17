//! Regression test for matched responses from multiple TestVM call mocks.

use alloy_primitives::Address;
use stylus_sdk::{
    stylus_core::host::{CalldataAccess, UnsafeCallAccess},
    testing::TestVM,
};

#[test]
fn mocked_static_calls_return_the_matched_response() {
    let vm = TestVM::new();
    let first_target = Address::from([1; 20]);
    let second_target = Address::from([2; 20]);
    let first_input = [0x11];
    let second_input = [0x22];
    let first_output = [0xaa];
    let second_output = [0xbb];

    vm.mock_static_call(first_target, first_input.to_vec(), Ok(first_output.to_vec()));
    vm.mock_static_call(second_target, second_input.to_vec(), Ok(second_output.to_vec()));

    let mut output_len = 0;
    let status = unsafe {
        vm.static_call_contract(
            first_target.as_ptr(),
            first_input.as_ptr(),
            first_input.len(),
            u64::MAX,
            &mut output_len,
        )
    };

    assert_eq!(status, 0);
    assert_eq!(output_len, first_output.len());
    assert_eq!(vm.read_return_data(0, None), first_output);
}
