use super::{
    ERC721OutOfBoundsIndex, Erc721Enumerable, Error, IErc721Enumerable,
};
use crate::test_support::{test_address, TestContract as TestVMContract};
use alloy_primitives::U256;

#[test]
fn max_index_cannot_return_the_owner_count_sentinel() {
    let contract =
        TestVMContract::<Erc721Enumerable>::new(stringify!(contract));
    let owner = test_address(stringify!(owner));

    // A valid nonempty owner's enumeration stores its token count at MAX.
    contract
        .sender(owner)
        .owned_tokens
        .setter(owner)
        .setter(U256::MAX)
        .set(U256::ONE);

    let err = contract
        .sender(owner)
        .token_of_owner_by_index(owner, U256::MAX)
        .expect_err("MAX is never a valid owner-token index");

    assert!(matches!(
        err,
        Error::OutOfBoundsIndex(ERC721OutOfBoundsIndex {
            owner: actual_owner,
            index
        }) if actual_owner == owner && index == U256::MAX
    ));
}

#[test]
fn legacy_owner_enumeration_without_count_metadata_remains_readable() {
    let contract =
        TestVMContract::<Erc721Enumerable>::new(stringify!(contract));
    let owner = test_address(stringify!(owner));
    let legacy_token_id = U256::from(7);

    // v0.3.0 stored token IDs by index without a separate owner-count entry.
    contract
        .sender(owner)
        .owned_tokens
        .setter(owner)
        .setter(U256::ZERO)
        .set(legacy_token_id);

    let token_id = contract
        .sender(owner)
        .token_of_owner_by_index(owner, U256::ZERO)
        .expect("legacy owner enumeration must remain readable after upgrade");

    assert_eq!(token_id, legacy_token_id);
}
