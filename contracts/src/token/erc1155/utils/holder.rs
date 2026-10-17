//! Implementation of the [`IErc1155Receiver`] trait.
use alloc::{vec, vec::Vec};

use alloy_primitives::{aliases::B32, Address, U256};
use stylus_sdk::{abi::Bytes, function_selector, prelude::*};

use crate::{
    token::erc1155::receiver::{
        IErc1155Receiver, BATCH_TRANSFER_FN_SELECTOR,
        SINGLE_TRANSFER_FN_SELECTOR,
    },
    utils::introspection::erc165::IErc165,
};

/// Simple implementation of [`IErc1155Receiver`] that will allow a contract to
/// hold ERC-1155 tokens.
#[storage]
pub struct Erc1155Holder;

#[public]
#[implements(IErc1155Receiver, IErc165)]
impl Erc1155Holder {}

#[public]
impl IErc1155Receiver for Erc1155Holder {
    #[selector(name = "onERC1155Received")]
    fn on_erc1155_received(
        &mut self,
        _operator: Address,
        _from: Address,
        _id: U256,
        _value: U256,
        _data: Bytes,
    ) -> Result<B32, Vec<u8>> {
        Ok(SINGLE_TRANSFER_FN_SELECTOR)
    }

    #[selector(name = "onERC1155BatchReceived")]
    fn on_erc1155_batch_received(
        &mut self,
        _operator: Address,
        _from: Address,
        _ids: Vec<U256>,
        _values: Vec<U256>,
        _data: Bytes,
    ) -> Result<B32, Vec<u8>> {
        Ok(BATCH_TRANSFER_FN_SELECTOR)
    }
}

#[public]
impl IErc165 for Erc1155Holder {
    fn supports_interface(&self, interface_id: B32) -> bool {
        <Self as IErc1155Receiver>::interface_id() == interface_id
            || <Self as IErc165>::interface_id() == interface_id
    }
}

#[cfg(test)]
mod tests {
    use stylus_sdk::testing::TestVM;

    use super::*;

    unsafe impl TopLevelStorage for Erc1155Holder {}

    #[test]
    fn holder_returns_proper_selectors() {
        let contract_vm = TestVM::default();
        let contract_address = Address::from([160u8; 20]);
        contract_vm.set_contract_address(contract_address);
        let mut contract = Erc1155Holder::from(&contract_vm);
        let alice = Address::from([1u8; 20]);

        let one = U256::from(1);
        assert_eq!(
            ({
                contract_vm.set_sender(alice);
                &mut contract
            })
            .on_erc1155_received(
                alice,
                alice,
                one,
                one,
                vec![].into()
            ),
            Ok(SINGLE_TRANSFER_FN_SELECTOR)
        );

        assert_eq!(
            ({
                contract_vm.set_sender(alice);
                &mut contract
            })
            .on_erc1155_batch_received(
                alice,
                alice,
                vec![one],
                vec![one],
                vec![].into()
            ),
            Ok(BATCH_TRANSFER_FN_SELECTOR)
        );
    }

    #[test]
    fn supports_interface() {
        let contract_vm = TestVM::default();
        let contract_address = Address::from([160u8; 20]);
        contract_vm.set_contract_address(contract_address);
        let mut contract = Erc1155Holder::from(&contract_vm);
        let alice = Address::from([1u8; 20]);

        assert!(({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .supports_interface(
            <Erc1155Holder as IErc1155Receiver>::interface_id()
        ));
        assert!(({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .supports_interface(<Erc1155Holder as IErc165>::interface_id()));

        let fake_interface_id: B32 = 0x12345678_u32.into();
        assert!(!({
            contract_vm.set_sender(alice);
            &mut contract
        })
        .supports_interface(fake_interface_id));
    }
}
