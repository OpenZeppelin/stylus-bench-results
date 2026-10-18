//! Implementation of the [`IErc721Receiver`] trait.
use alloc::{vec, vec::Vec};

use alloy_primitives::{aliases::B32, Address, U256};
use stylus_sdk::{abi::Bytes, function_selector, prelude::*};

use crate::token::erc721::receiver::{IErc721Receiver, RECEIVER_FN_SELECTOR};

/// Default implementation of the [`IErc721Receiver`] trait.
#[storage]
pub struct Erc721Holder;

#[public]
#[implements(IErc721Receiver)]
impl Erc721Holder {}

#[public]
impl IErc721Receiver for Erc721Holder {
    #[selector(name = "onERC721Received")]
    fn on_erc721_received(
        &mut self,
        _operator: Address,
        _from: Address,
        _token_id: U256,
        _data: Bytes,
    ) -> Result<B32, Vec<u8>> {
        Ok(RECEIVER_FN_SELECTOR)
    }
}

#[cfg(test)]
mod tests {
    use crate::test_support::{test_address, TestContract as TestVMContract};

    use super::*;

    unsafe impl TopLevelStorage for Erc721Holder {}

    #[test]
    fn holder_returns_proper_selector() {
        let contract =
            TestVMContract::<Erc721Holder>::new(stringify!(contract));
        let alice = test_address(stringify!(alice));

        assert_eq!(
            contract.sender(alice).on_erc721_received(
                alice,
                alice,
                U256::from(1),
                vec![].into()
            ),
            Ok(RECEIVER_FN_SELECTOR)
        );
    }
}
