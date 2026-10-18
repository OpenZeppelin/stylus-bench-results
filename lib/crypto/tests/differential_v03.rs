//! Golden vectors collected from OpenZeppelin rust-contracts-stylus v0.3.0.

use openzeppelin_crypto::{
    eddsa::{SecretKey, SigningKey, SECRET_KEY_LENGTH},
    field::instance::FpBN256,
    hash::{commutative_hash_pair, BuildHasher, Hasher},
    merkle::Verifier,
    poseidon2::{instance::bn256::BN256Params, Poseidon2},
    KeccakBuilder,
};

#[test]
fn cryptographic_outputs_match_v0_3_0_vectors() {
    let builder = KeccakBuilder;
    let mut hasher = builder.build_hasher();
    hasher.update(b"");
    assert_eq!(
        hex(&hasher.finalize()),
        "c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470"
    );

    let mut hasher = builder.build_hasher();
    hasher.update(b"Stylus migration vector");
    assert_eq!(
        hex(&hasher.finalize()),
        "911072c7156d711d4b9d3e08f3ed4369de8d9768db03e5c4435c121a74be0f89"
    );

    let secret_key: SecretKey = [1; SECRET_KEY_LENGTH];
    let signing_key = SigningKey::from_bytes(&secret_key);
    let message = b"Stylus migration differential test";
    assert_eq!(
        hex(&signing_key.verifying_key().to_bytes()),
        "8a88e3dd7409f195fd52db2d3cba5d72ca6709bf1d94121bf3748801b40f6f5c"
    );
    assert_eq!(
        hex(&signing_key.sign(message).to_bytes()),
        "2cbbe3c198770cb5a8ff369ff324f4e6293866e8bd1bf9b2cea249ca00017baf867aef843a84d0b11fd950ba34bd345aedd096e1e738002f860896090659420e"
    );

    let left = [0x11; 32];
    let right = [0x22; 32];
    let root = commutative_hash_pair(&left, &right, builder.build_hasher());
    assert!(Verifier::verify(&[right], root, left));
    assert_eq!(
        hex(&root),
        "3e92e0db88d6afea9edc4eedf62fffa4d92bcdfc310dccbe943747fe8302e871"
    );

    let mut poseidon = Poseidon2::<BN256Params, FpBN256>::new();
    poseidon.absorb_batch(&[
        FpBN256::from(1_u64),
        FpBN256::from(2_u64),
        FpBN256::from(3_u64),
    ]);
    assert_eq!(
        poseidon.squeeze().to_string(),
        "1573C000A10B74FE7922D5FC079B5B0147FFFF2260028DFED14D5706BE0DDD36"
    );
}

fn hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[usize::from(byte >> 4)] as char);
        output.push(HEX[usize::from(byte & 0x0f)] as char);
    }
    output
}
