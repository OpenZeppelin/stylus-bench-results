use alloy_primitives::{B16, U256};
use openzeppelin_stylus_proc::interface_id;

#[interface_id]
trait CollidingSelectors {
    fn burn(&self, amount: U256);

    #[selector(name = "collate_propagate_storage")]
    fn collate_propagate_storage(&self, value: B16);
}

fn main() {}
