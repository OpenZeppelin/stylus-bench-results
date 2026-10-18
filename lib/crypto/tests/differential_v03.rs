//! Regression vectors captured by running the v0.3.0 crypto crate.

use openzeppelin_crypto::{
    eddsa::{SigningKey, SECRET_KEY_LENGTH},
    field::instance::FpBN256,
    hash::{commutative_hash_pair, BuildHasher, Hasher},
    merkle::Verifier,
    poseidon2::{instance::bn256::BN256Params, Poseidon2},
    KeccakBuilder,
};

#[test]
fn cryptographic_outputs_match_v0_3_0_vectors() {
    let keccak_vectors: &[(&[u8], [u8; 32])] = &[
        (
            b"",
            hex_literal::hex!(
                "c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"
            ),
        ),
        (
            b"openzeppelin-stylus-v0.3.0",
            hex_literal::hex!(
                "dc32c933ec8465d0de24cb392867911c221924bc3fb6a628f91d0776bf4b4375"
            ),
        ),
        (
            &(0u8..=255).collect::<Vec<_>>(),
            hex_literal::hex!(
                "dc924469b334aed2a19fac7252e9961aea41f8d91996366029dbe0884229bf36"
            ),
        ),
    ];
    for (message, expected) in keccak_vectors {
        let mut hasher = KeccakBuilder.build_hasher();
        hasher.update(message);
        assert_eq!(&hasher.finalize(), expected);
    }

    let secret_key = [7u8; SECRET_KEY_LENGTH];
    let signing_key = SigningKey::from_bytes(&secret_key);
    assert_eq!(
        signing_key.verifying_key().to_bytes(),
        hex_literal::hex!(
            "ea4a6c63e29c520abef5507b132ec5f9954776aebebe7b92421eea691446d22c"
        ),
    );
    let message = b"SDK migration differential vector";
    let signature = signing_key.sign(message);
    assert_eq!(
        signature.to_bytes(),
        hex_literal::hex!(
            "8e457ce86208ff8f78a0664da6b8fda31bb2239e3cf344d0a52e089c316556995b78074edde08ee62773bb6bac05702c6fd7d083f4e94252320e454da588050c"
        ),
    );
    assert!(signing_key.is_valid_signature(message, &signature));

    let leaf = [0x11; 32];
    let proof = [0x22; 32];
    let root =
        commutative_hash_pair(&leaf, &proof, KeccakBuilder.build_hasher());
    assert_eq!(
        root,
        hex_literal::hex!(
            "3e92e0db88d6afea9edc4eedf62fffa4d92bcdfc310dccbe943747fe8302e871"
        ),
    );
    assert!(Verifier::verify(&[proof], root, leaf));
    assert!(!Verifier::verify(&[[0x23; 32]], root, leaf));

    let mut poseidon = Poseidon2::<BN256Params, FpBN256>::new();
    for input in [1u64, 2, 3, 5, 8, 13, 21] {
        poseidon.absorb(&FpBN256::from(input));
    }
    assert_eq!(
        poseidon.squeeze().to_string(),
        "1512295DEFA9B26D79B46490F652CCB1F37E443FE7919622491BDBDF0FBE3157",
    );
}
