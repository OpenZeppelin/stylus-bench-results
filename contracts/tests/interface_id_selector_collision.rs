#[test]
fn rejects_distinct_signatures_with_the_same_selector() {
    let tests = trybuild::TestCases::new();
    tests.compile_fail("tests/ui/interface_id_selector_collision.rs");
}
