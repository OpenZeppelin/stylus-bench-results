#![cfg_attr(not(any(test, feature = "export-abi")), no_main)]
#![allow(clippy::needless_pass_by_value, clippy::unused_self)]

extern crate alloc;

use openzeppelin_crypto::{
    curve::te::instance::curve25519::{Curve25519FqParam, Curve25519FrParam},
    eddsa::{AffinePoint, Scalar, Signature, VerifyingKey},
    field::fp::{Fp256, FpParams},
};
use stylus_sdk::{abi::Bytes, prelude::*};

#[entrypoint]
#[storage]
struct EddsaExample;

fn has_canonical_encoding(
    verifying_key: [alloy_primitives::U256; 2],
    signature: [alloy_primitives::U256; 3],
) -> bool {
    let base_modulus: alloy_primitives::U256 =
        Curve25519FqParam::MODULUS.into();
    let scalar_modulus: alloy_primitives::U256 =
        Curve25519FrParam::MODULUS.into();

    verifying_key.into_iter().all(|coordinate| coordinate < base_modulus)
        && signature[..2].iter().all(|coordinate| *coordinate < base_modulus)
        && signature[2] < scalar_modulus
}

#[public]
impl EddsaExample {
    /// Verification api is slightly different from canonical implementation
    /// missing compressed points. Coordinates must be less than the base-field
    /// modulus, and `s` must be less than the scalar-field modulus.
    fn verify(
        &self,
        verifying_key: [alloy_primitives::U256; 2],
        signature: [alloy_primitives::U256; 3],
        message: Bytes,
    ) -> bool {
        if !has_canonical_encoding(verifying_key, signature) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_noncanonical_coordinates_and_scalar() {
        let field_modulus: alloy_primitives::U256 =
            Curve25519FqParam::MODULUS.into();
        let scalar_modulus: alloy_primitives::U256 =
            Curve25519FrParam::MODULUS.into();
        let one = alloy_primitives::U256::from(1u8);

        assert!(has_canonical_encoding(
            [field_modulus - one, field_modulus - one],
            [field_modulus - one, field_modulus - one, scalar_modulus - one],
        ));
        assert!(!has_canonical_encoding(
            [field_modulus, alloy_primitives::U256::ZERO],
            [alloy_primitives::U256::ZERO; 3],
        ));
        assert!(!has_canonical_encoding(
            [alloy_primitives::U256::ZERO; 2],
            [
                alloy_primitives::U256::ZERO,
                alloy_primitives::U256::ZERO,
                scalar_modulus,
            ],
        ));
    }
}
