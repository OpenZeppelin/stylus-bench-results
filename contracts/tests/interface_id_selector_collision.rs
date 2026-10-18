//! Compile-time regression test for colliding Solidity function selectors.

#[test]
fn fixture_signatures_have_identical_selectors() {
    assert_eq!(
        stylus_sdk::function_selector!("burn", alloy_primitives::U256),
        stylus_sdk::function_selector!(
            "collate_propagate_storage",
            alloy_primitives::aliases::B128
        )
    );
}

#[test]
fn rejects_distinct_signatures_with_the_same_selector() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/interface_id_selector_collision.rs");
}
