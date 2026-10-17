//! Tests conditional method selectors in generated ERC-165 IDs.

use alloy_primitives::aliases::B32;
use openzeppelin_stylus_proc::interface_id;

#[interface_id]
trait ConditionalInterface {
    #[cfg(feature = "conditional-selector")]
    fn optional(&self);
}

struct Contract;

impl ConditionalInterface for Contract {
    #[cfg(feature = "conditional-selector")]
    fn optional(&self) {}
}

#[test]
fn interface_id_only_contains_compiled_methods() {
    let expected = if cfg!(feature = "conditional-selector") {
        B32::new(stylus_sdk::function_selector!("optional"))
    } else {
        B32::new([0; 4])
    };

    assert_eq!(<Contract as ConditionalInterface>::interface_id(), expected);
}
