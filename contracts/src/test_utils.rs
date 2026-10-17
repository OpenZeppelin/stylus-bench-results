//! Helpers shared by the contract unit tests.

use alloy_sol_types::SolEvent;
use stylus_sdk::testing::TestVM;

/// Asserts that `vm` emitted `expected` as a Solidity event.
pub(crate) fn assert_emitted<E: SolEvent>(vm: &TestVM, expected: &E) {
    let expected_topics: Vec<alloy_primitives::B256> =
        expected.encode_topics().into_iter().map(Into::into).collect();
    let expected_data = expected.encode_data();

    assert!(vm.get_emitted_logs().iter().any(|(topics, data)| {
        topics == &expected_topics && data == &expected_data
    }));
}
