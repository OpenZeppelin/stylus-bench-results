//! Regression test for BeaconProxy storage isolation.

extern crate alloc;

use alloy_primitives::Address;
use alloy_sol_types::{sol, SolCall, SolValue};
use openzeppelin_stylus::proxy::beacon::BeaconProxy;
use stylus_sdk::{
    abi::Bytes,
    prelude::*,
    storage::StorageAddress,
    testing::TestVM,
};

sol! {
    interface Beacon {
        function implementation() external view returns (address);
    }
}

#[storage]
struct ScalarImplementation {
    owner: StorageAddress,
}

unsafe impl TopLevelStorage for ScalarImplementation {}

#[test]
fn implementation_scalar_must_not_overwrite_beacon_address() {
    let vm = TestVM::default();
    let proxy_address = Address::from([0x10; 20]);
    let beacon = Address::from([0x20; 20]);
    let implementation = Address::from([0x30; 20]);
    let owner = Address::from([0x40; 20]);
    vm.set_contract_address(proxy_address);
    vm.set_code(beacon, vec![1]);
    vm.set_code(implementation, vec![1]);
    vm.mock_static_call(
        beacon,
        Beacon::implementationCall {}.abi_encode(),
        Ok(implementation.abi_encode()),
    );

    let mut proxy = BeaconProxy::from(&vm);
    proxy
        .constructor(beacon, &Bytes::new())
        .expect("beacon proxy initialization should succeed");
    assert_eq!(proxy.get_beacon(), beacon);

    let mut implementation_state = ScalarImplementation::from(&vm);
    implementation_state.owner.set(owner);

    assert_eq!(
        proxy.get_beacon(),
        beacon,
        "implementation storage at slot zero must not change the beacon"
    );
}
