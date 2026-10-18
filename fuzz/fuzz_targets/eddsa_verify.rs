#![no_main]

use libfuzzer_sys::fuzz_target;
use openzeppelin_crypto::{
    arithmetic::uint::U256,
    curve::te::{
        instance::curve25519::{Curve25519Config, Fq, Fr},
        Projective,
    },
    eddsa::{Signature, VerifyingKey},
};

fn field(bytes: &[u8]) -> Fq {
    Fq::from_bigint(U256::from_bytes_le(bytes))
}

fn point(bytes: &[u8]) -> Projective<Curve25519Config> {
    Projective {
        x: field(&bytes[0..32]),
        y: field(&bytes[32..64]),
        z: field(&bytes[96..128]),
        t: field(&bytes[64..96]),
    }
}

fuzz_target!(|data: &[u8]| {
    // Two arbitrary extended points and one scalar exercise zero-Z, invalid
    // curve, invalid subgroup, and inconsistent-coordinate verifier paths.
    if data.len() < 288 {
        return;
    }

    let verifying_key = VerifyingKey { point: point(&data[0..128]) };
    let signature = Signature {
        R: point(&data[128..256]),
        s: Fr::from_bigint(U256::from_bytes_le(&data[256..288])),
    };

    let _ = verifying_key.is_valid(&data[288..], &signature);
});
