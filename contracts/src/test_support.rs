use std::{
    cell::{Cell, RefCell},
    fmt::Debug,
    ops::{Deref, DerefMut},
    sync::atomic::{AtomicU64, Ordering},
};

use alloy_primitives::{keccak256, Address, U256};
use alloy_sol_types::SolEvent;
use stylus_sdk::testing::{state::VMState, TestVM};

thread_local! {
    static LAST_TRANSACTION: RefCell<Option<(std::rc::Rc<RefCell<TestVM>>, VMState)>> = const { RefCell::new(None) };
    static CONTRACT_ADDRESSES: RefCell<Vec<Address>> = const { RefCell::new(Vec::new()) };
    static CALL_DEPTH: Cell<usize> = const { Cell::new(0) };
}

static NEXT_ADDRESS: AtomicU64 = AtomicU64::new(1);

/// Creates stable test addresses and contracts from readable parameter names.
pub(crate) trait FromTag: Sized {
    /// Create an entity from its test parameter name.
    fn from_tag(tag: &str) -> Self;
}

impl FromTag for Address {
    fn from_tag(tag: &str) -> Self {
        Address::from_slice(&keccak256(tag.as_bytes())[..20])
    }
}

/// One storage contract connected to an SDK `TestVM`.
pub(crate) struct Contract<T> {
    address: Address,
    vm: std::rc::Rc<RefCell<TestVM>>,
    _value: std::marker::PhantomData<T>,
}

impl<T> Contract<T>
where
    for<'a> T: From<&'a TestVM>,
{
    /// Create a contract at a process-unique test address.
    pub(crate) fn new() -> Self {
        let mut seed = [0; 32];
        seed[24..].copy_from_slice(
            &NEXT_ADDRESS.fetch_add(1, Ordering::Relaxed).to_be_bytes(),
        );
        Self::new_at(Address::from_slice(&keccak256(seed)[..20]))
    }

    /// Create a contract at a deterministic address.
    pub(crate) fn new_at(address: Address) -> Self {
        CONTRACT_ADDRESSES
            .with(|addresses| addresses.borrow_mut().push(address));
        let vm = TestVM::new();
        vm.set_contract_address(address);
        Self {
            address,
            vm: std::rc::Rc::new(RefCell::new(vm)),
            _value: std::marker::PhantomData,
        }
    }

    /// Address assigned to this test contract.
    pub(crate) fn address(&self) -> Address {
        self.address
    }

    /// SDK `TestVM` used by the fixture.
    pub(crate) fn vm(&self) -> TestVM {
        self.vm.borrow().clone()
    }

    /// Set the fixture VM balance of `address`.
    pub(crate) fn fund(&self, address: Address, amount: U256) {
        self.vm.borrow().set_balance(address, amount);
    }

    /// Start a call with `sender` as the message sender.
    pub(crate) fn sender(&self, sender: Address) -> ContractCall<'_, T> {
        self.call(sender, U256::ZERO)
    }

    /// Start a call with a sender and attached value.
    pub(crate) fn sender_and_value(
        &self,
        sender: Address,
        value: U256,
    ) -> ContractCall<'_, T> {
        self.call(sender, value)
    }

    fn call(&self, sender: Address, value: U256) -> ContractCall<'_, T> {
        let vm = self.vm.borrow().clone();
        CONTRACT_ADDRESSES.with(|addresses| {
            for address in addresses.borrow().iter().copied() {
                if address != self.address {
                    vm.set_code(address, vec![0x00]);
                }
            }
        });
        vm.set_contract_address(self.address);
        vm.set_sender(sender);
        vm.set_value(value);
        CALL_DEPTH.with(|depth| {
            if depth.get() == 0 {
                LAST_TRANSACTION.with(|state| {
                    *state.borrow_mut() =
                        Some((self.vm.clone(), vm.snapshot()));
                });
            }
            depth.set(depth.get() + 1);
        });
        ContractCall {
            value: T::from(&vm),
            _contract: std::marker::PhantomData,
        }
    }

    /// Assert that `event` appears in the SDK `TestVM`'s emitted logs.
    pub(crate) fn assert_emitted<E: SolEvent + Debug>(&self, event: &E) {
        let actual = self.vm.borrow().get_emitted_logs();
        assert!(
            actual.iter().any(|(topics, data)| {
                let expected = event.encode_log_data();
                topics == expected.topics() && data.as_slice() == expected.data.as_ref()
            }),
            "expected event was not emitted: {event:?}; actual logs: {actual:?}"
        );
    }
}

/// Reset thread-local fixture state before each parameterized test.
pub(crate) fn reset_test_state() {
    CONTRACT_ADDRESSES.with(|addresses| addresses.borrow_mut().clear());
    LAST_TRANSACTION.with(|state| *state.borrow_mut() = None);
    CALL_DEPTH.with(|depth| depth.set(0));
}

impl<T> FromTag for Contract<T>
where
    for<'a> T: From<&'a TestVM>,
{
    fn from_tag(tag: &str) -> Self {
        Self::new_at(Address::from_tag(tag))
    }
}

/// A mutable contract call context that sets the SDK `TestVM` message fields.
pub(crate) struct ContractCall<'a, T> {
    value: T,
    _contract: std::marker::PhantomData<&'a Contract<T>>,
}

impl<T> Deref for ContractCall<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T> DerefMut for ContractCall<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.value
    }
}

impl<T> Drop for ContractCall<'_, T> {
    fn drop(&mut self) {
        CALL_DEPTH.with(|depth| depth.set(depth.get().saturating_sub(1)));
    }
}

/// Result assertions used by SDK `TestVM` unit tests.
pub(crate) trait TransactionResultExt<T, E> {
    /// Return the error or panic with `message` if the operation succeeded.
    fn expect_error(self, message: &str) -> E;

    /// Return the error or panic if the operation succeeded.
    fn unwrap_error(self) -> E;
}

impl<T, E: Debug> TransactionResultExt<T, E> for Result<T, E> {
    #[track_caller]
    fn expect_error(self, message: &str) -> E {
        match self {
            Ok(_) => panic!("{message}: operation succeeded unexpectedly"),
            Err(error) => {
                rollback_last_transaction();
                error
            }
        }
    }

    #[track_caller]
    fn unwrap_error(self) -> E {
        self.expect_error("called unwrap_error on a successful result")
    }
}

fn rollback_last_transaction() {
    LAST_TRANSACTION.with(|state| {
        if let Some((vm, snapshot)) = state.borrow_mut().take() {
            *vm.borrow_mut() = TestVM::from(snapshot);
        }
    });
}
