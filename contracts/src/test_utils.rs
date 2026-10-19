//! Fixtures shared by the unit tests.
//!
//! The Stylus testing framework ([`TestVM`](stylus_sdk::testing::TestVM)) is
//! given explicit addresses, so tests use these constants where motsu used to
//! inject random accounts.

use alloy_primitives::{address, Address};

pub(crate) const ALICE: Address =
    address!("0x000000000000000000000000000000000000a11c");
pub(crate) const BOB: Address =
    address!("0x0000000000000000000000000000000000000b0b");
pub(crate) const CHARLIE: Address =
    address!("0x00000000000000000000000000000000000c4a1e");
pub(crate) const DAVE: Address =
    address!("0x000000000000000000000000000000000000da4e");
