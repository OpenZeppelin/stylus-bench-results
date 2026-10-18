//! Small adapters around the Stylus SDK's test VM for unit tests.

use core::ops::{Deref, DerefMut};
use std::{cell::Cell, cell::RefCell, cell::RefMut, fmt::Debug};

use alloy_primitives::{keccak256, Address, U256};
use alloy_sol_types::{SolEvent, SolValue};
use stylus_sdk::storage::StorageType;
use stylus_test::{constants::DEFAULT_SENDER, TestVM, TestVMBuilder};

/// Creates the same deterministic address for a test name across runs.
pub(crate) fn test_address(tag: &str) -> Address {
    let hash = keccak256(tag.as_bytes());
    Address::from_slice(&hash[..20])
}

/// Gives a mock address code so SDK call helpers treat it as a contract.
pub(crate) fn mock_code(vm: &TestVM, address: Address) {
    vm.set_code(address, vec![0x00]);
}

/// Registers an ABI-encoded mock contract call.
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn mock_call<T: SolValue>(
    vm: &TestVM,
    address: Address,
    signature: &str,
    args: T,
    result: Result<Vec<u8>, Vec<u8>>,
) {
    let mut data = keccak256(signature.as_bytes())[..4].to_vec();
    data.extend(args.abi_encode());
    vm.mock_call(address, data, U256::ZERO, result);
}

/// Registers an ABI-encoded mock static contract call.
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn mock_static_call<T: SolValue>(
    vm: &TestVM,
    address: Address,
    signature: &str,
    args: T,
    result: Result<Vec<u8>, Vec<u8>>,
) {
    let mut data = keccak256(signature.as_bytes())[..4].to_vec();
    data.extend(args.abi_encode());
    vm.mock_static_call(address, data, result);
}

/// Registers an ABI-encoded mock call whose arguments are already encoded as
/// a parameter sequence.
pub(crate) fn mock_call_with_encoded_args(
    vm: &TestVM,
    address: Address,
    signature: &str,
    args: &[u8],
    result: Result<Vec<u8>, Vec<u8>>,
) {
    let mut data = keccak256(signature.as_bytes())[..4].to_vec();
    data.extend(args);
    vm.mock_call(address, data, U256::ZERO, result);
}

/// A storage instance with explicit VM and call context for unit tests.
pub(crate) struct TestContract<T> {
    vm: TestVM,
    storage: RefCell<T>,
    address: Address,
    sender: Cell<Address>,
    value: Cell<U256>,
}

impl<T> TestContract<T>
where
    T: StorageType + for<'a> From<&'a TestVM>,
{
    /// Creates an isolated storage instance at a deterministic test address.
    pub(crate) fn new(tag: &str) -> Self {
        let address = test_address(tag);
        let vm = TestVMBuilder::new().contract_address(address).build();
        Self {
            storage: RefCell::new(T::from(&vm)),
            vm,
            address,
            sender: Cell::new(DEFAULT_SENDER),
            value: Cell::new(U256::ZERO),
        }
    }

    /// Returns a call guard with the requested sender and zero call value.
    pub(crate) fn sender(&self, sender: Address) -> TestContractCall<'_, T> {
        self.call(sender, U256::ZERO)
    }

    /// Returns a call guard with the requested sender and value.
    pub(crate) fn sender_and_value(
        &self,
        sender: Address,
        value: U256,
    ) -> TestContractCall<'_, T> {
        self.call(sender, value)
    }

    fn call(&self, sender: Address, value: U256) -> TestContractCall<'_, T> {
        let previous_sender = self.sender.replace(sender);
        let previous_value = self.value.replace(value);
        self.vm.set_sender(sender);
        self.vm.set_value(value);
        TestContractCall {
            contract: self,
            storage: self.storage.borrow_mut(),
            previous_sender,
            previous_value,
        }
    }

    /// Returns the test contract's configured address.
    pub(crate) fn address(&self) -> Address {
        self.address
    }

    /// Returns the underlying SDK test VM for setting host mocks.
    pub(crate) fn vm(&self) -> &TestVM {
        &self.vm
    }

    /// Asserts that the expected event was emitted by this test contract.
    pub(crate) fn assert_emitted<E: SolEvent + Debug>(&self, event: &E) {
        let expected = event.encode_log_data();
        assert!(
            self.vm.get_emitted_logs().iter().any(|(topics, data)| {
                topics.as_slice() == expected.topics()
                    && data.as_slice() == expected.data.as_ref()
            }),
            "event was not emitted: {event:?}"
        );
    }
}

/// A contract call that restores the previous VM sender and value on drop.
pub(crate) struct TestContractCall<'a, T> {
    contract: &'a TestContract<T>,
    storage: RefMut<'a, T>,
    previous_sender: Address,
    previous_value: U256,
}

impl<T> Deref for TestContractCall<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.storage
    }
}

impl<T> DerefMut for TestContractCall<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.storage
    }
}

impl<T> TestContractCall<'_, T> {
    /// Returns the composed test contract storage for disjoint field access.
    pub(crate) fn storage_mut(&mut self) -> &mut T {
        &mut self.storage
    }
}

impl<T> Drop for TestContractCall<'_, T> {
    fn drop(&mut self) {
        self.contract.sender.set(self.previous_sender);
        self.contract.value.set(self.previous_value);
        self.contract.vm.set_sender(self.previous_sender);
        self.contract.vm.set_value(self.previous_value);
    }
}
