//! Fixtures shared by the unit tests.
//!
//! The Stylus testing framework ([`TestVM`](stylus_sdk::testing::TestVM))
//! doesn't provide accounts, so the tests use these fixed addresses as
//! senders, owners, spenders, recipients, etc.

use alloy_primitives::{address, Address};

pub(crate) const ALICE: Address =
    address!("0x000000000000000000000000000000000000a11c");
pub(crate) const BOB: Address =
    address!("0x0000000000000000000000000000000000000b0b");
pub(crate) const CHARLIE: Address =
    address!("0x00000000000000000000000000000000000c4a1e");
pub(crate) const DAVE: Address =
    address!("0x000000000000000000000000000000000000da4e");
