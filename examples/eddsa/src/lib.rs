#![cfg_attr(not(any(test, feature = "export-abi")), no_main)]
#![allow(clippy::needless_pass_by_value, clippy::unused_self)]

extern crate alloc;

use alloc::vec::Vec;

use alloy_primitives::{uint, U256};
use openzeppelin_crypto::{
    curve::te::instance::curve25519::Curve25519FqParam,
    eddsa::{AffinePoint, Scalar, Signature, VerifyingKey},
    field::fp::Fp256,
};
use stylus_sdk::{abi::Bytes, prelude::*};

const FIELD_MODULUS: U256 = uint!(
    57896044618658097711785492504343953926634992332820282019728792003956564819949_U256
);
const SCALAR_MODULUS: U256 = uint!(
    7237005577332262213973186563042994240857116359379907606001950938285454250989_U256
);

#[entrypoint]
#[storage]
struct EddsaExample;

#[public]
impl EddsaExample {
    /// Verification api is slightly different from canonical implementation
    /// missing compressed points.
    fn verify(
        &self,
        verifying_key: [alloy_primitives::U256; 2],
        signature: [alloy_primitives::U256; 3],
        message: Bytes,
    ) -> bool {
        if verifying_key.iter().any(|coordinate| *coordinate >= FIELD_MODULUS)
            || signature[..2]
                .iter()
                .any(|coordinate| *coordinate >= FIELD_MODULUS)
            || signature[2] >= SCALAR_MODULUS
        {
            return false;
        }

        let verifying_key = VerifyingKey::from_affine(AffinePoint {
            x: Fp256::<Curve25519FqParam>::from_bigint(verifying_key[0].into()),
            y: Fp256::<Curve25519FqParam>::from_bigint(verifying_key[1].into()),
        });

        let signature = Signature::from_affine_R_s(
            AffinePoint {
                x: Fp256::<Curve25519FqParam>::from_bigint(signature[0].into()),
                y: Fp256::<Curve25519FqParam>::from_bigint(signature[1].into()),
            },
            Scalar::from_bigint(signature[2].into()),
        );

        verifying_key.is_valid(&message, &signature)
    }
}
