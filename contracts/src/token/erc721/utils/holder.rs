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
    use stylus_sdk::testing::TestVM;

    use super::*;

    unsafe impl TopLevelStorage for Erc721Holder {}

    #[test]
    fn holder_returns_proper_selector() {
        let contract_vm = TestVM::default();
        let contract_address = Address::from([160u8; 20]);
        contract_vm.set_contract_address(contract_address);
        let mut contract = Erc721Holder::from(&contract_vm);
        let alice = Address::from([1u8; 20]);

        assert_eq!(
            ({
                contract_vm.set_sender(alice);
                &mut contract
            })
            .on_erc721_received(
                alice,
                alice,
                U256::from(1),
                vec![].into()
            ),
            Ok(RECEIVER_FN_SELECTOR)
        );
    }
}
