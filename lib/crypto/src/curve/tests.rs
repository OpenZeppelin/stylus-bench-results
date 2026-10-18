// Explicit references here exercise the macro-generated borrowed operator
// implementations; clippy's op_ref lint would otherwise erase that coverage.
#![allow(clippy::op_ref)]

use num_traits::Zero;
use proptest::proptest;

use super::{
    sw::{
        instance::{
            secp256k1::{Fq as Secp256k1Fq, Secp256k1Config},
            starknet::StarknetCurveConfig,
        },
        Affine as SWAffine, Projective as SWProjective, SWCurveConfig,
    },
    te::{
        instance::{
            baby_jubjub::{BabyJubjubConfig, Fq as BabyJubjubFq},
            bandersnatch::BandersnatchConfig,
            curve25519::Curve25519Config,
            jubjub::JubjubConfig,
        },
        Affine as TEAffine, Projective as TEProjective, TECurveConfig,
    },
    AffineRepr, CurveGroup,
};
use crate::field::{group::AdditiveGroup, Field};

fn check_sw_group<P: SWCurveConfig>(left: u64, right: u64, scale: u8) {
    let generator = SWAffine::<P>::generator();
    let left_point = generator.mul_bigint(left);
    let right_point = generator.mul_bigint(right);
    let expected = generator.mul_bigint(left + right);
    let expected_difference = if left >= right {
        generator.mul_bigint(left - right)
    } else {
        -generator.mul_bigint(right - left)
    };

    // The group law must agree across affine, mixed, projective and scalar
    // multiplication APIs, including identity and inverse cases.
    assert_eq!(left_point + right_point, expected);
    assert_eq!(left_point + right_point.into_affine(), expected);
    assert_eq!(left_point - right_point, expected_difference);
    assert_eq!(left_point + -left_point, SWProjective::<P>::zero());
    assert_eq!(left_point + SWProjective::<P>::zero(), left_point);
    assert_eq!(SWProjective::<P>::zero() + left_point, left_point);
    assert_eq!(left_point + SWAffine::<P>::zero(), left_point);
    assert_eq!(SWAffine::<P>::zero() + left_point, left_point);

    let mut doubled = left_point;
    doubled.double_in_place();
    assert_eq!(doubled, left_point + left_point);
    assert_eq!(left_point.double(), left_point + left_point);

    let left_affine = left_point.into_affine();
    assert_eq!(
        left_affine.xy(),
        (!left_affine.infinity).then_some((left_affine.x, left_affine.y))
    );
    assert_eq!(left_affine.is_zero(), left == 0);
    assert_eq!(left_affine.into_group(), left_point);
    assert_eq!(SWProjective::<P>::from(left_affine).into_affine(), left_affine);

    // Equivalent Jacobian triples use X/Z^2 and Y/Z^3.
    let scale = P::BaseField::from(u64::from(scale.max(1)));
    let scale_squared = scale.square();
    let scaled = SWProjective::<P>::new_unchecked(
        left_point.x * scale_squared,
        left_point.y * scale_squared * scale,
        left_point.z * scale,
    );
    assert_eq!(scaled, left_point);
    assert_eq!(scaled.into_affine(), left_affine);

    let normalized = SWProjective::<P>::normalize_batch(&[
        SWProjective::<P>::zero(),
        left_point,
        scaled,
    ]);
    assert_eq!(normalized, [SWAffine::<P>::zero(), left_affine, left_affine]);
    assert!(SWProjective::<P>::normalize_batch(&[]).is_empty());

    // Exercise the reference-forwarding operator implementations as well as
    // the standard owned operators.
    let mut right_copy = right_point;
    assert_eq!(&left_point + right_point, expected);
    assert_eq!(&left_point + &right_point, expected);
    assert_eq!(&left_point + &mut right_copy, expected);
    assert_eq!(left_point + &mut right_copy, expected);
    assert_eq!(&left_point - right_point, expected_difference);
    assert_eq!(&left_point - &right_point, expected_difference);
    assert_eq!(&left_point - &mut right_copy, expected_difference);

    let mut assigned = left_point;
    assigned += right_point;
    assert_eq!(assigned, expected);
    assigned -= right_point;
    assert_eq!(assigned, left_point);
    let mut assigned_mut = left_point;
    let mut rhs_mut = right_point;
    assigned_mut += &mut rhs_mut;
    assert_eq!(assigned_mut, expected);
    assigned_mut -= &mut rhs_mut;
    assert_eq!(assigned_mut, left_point);

    let affine_sum: SWProjective<P> =
        [left_affine, right_point.into_affine()].into_iter().sum();
    assert_eq!(affine_sum, expected);
    let affine_ref_sum: SWProjective<P> =
        [&left_affine, &right_point.into_affine()].into_iter().sum();
    assert_eq!(affine_ref_sum, expected);
    let projective_ref_sum: SWProjective<P> =
        [&left_point, &right_point].into_iter().sum();
    assert_eq!(projective_ref_sum, expected);

    assert_eq!(
        left_point * P::ScalarField::from(0u64),
        SWProjective::<P>::zero()
    );
    assert_eq!(left_point * P::ScalarField::from(1u64), left_point);
    assert_eq!(
        left_point * P::ScalarField::from(2u64),
        left_point + left_point
    );
}

fn check_te_group<P: TECurveConfig>(left: u64, right: u64, scale: u8) {
    let generator = TEAffine::<P>::generator();
    let left_point = generator.mul_bigint(left);
    let right_point = generator.mul_bigint(right);
    let expected = generator.mul_bigint(left + right);
    let expected_difference = if left >= right {
        generator.mul_bigint(left - right)
    } else {
        -generator.mul_bigint(right - left)
    };

    assert_eq!(left_point + right_point, expected);
    assert_eq!(left_point + right_point.into_affine(), expected);
    assert_eq!(left_point - right_point, expected_difference);
    assert_eq!(left_point + -left_point, TEProjective::<P>::zero());
    assert_eq!(left_point + TEProjective::<P>::zero(), left_point);
    assert_eq!(TEProjective::<P>::zero() + left_point, left_point);
    assert_eq!(left_point + TEAffine::<P>::zero(), left_point);
    assert_eq!(TEAffine::<P>::zero() + left_point, left_point);

    let mut doubled = left_point;
    doubled.double_in_place();
    assert_eq!(doubled, left_point + left_point);
    assert_eq!(left_point.double(), left_point + left_point);

    let left_affine = left_point.into_affine();
    assert_eq!(
        left_affine.xy(),
        (!left_affine.is_zero()).then_some((left_affine.x, left_affine.y))
    );
    assert_eq!(left_affine.is_zero(), left == 0);
    assert_eq!(left_affine.into_group(), left_point);
    assert_eq!(TEProjective::<P>::from(left_affine).into_affine(), left_affine);

    // Equivalent extended Edwards coordinates scale X, Y, T and Z together.
    let scale = P::BaseField::from(u64::from(scale.max(1)));
    let scaled = TEProjective::<P>::new_unchecked(
        left_point.x * scale,
        left_point.y * scale,
        left_point.t * scale,
        left_point.z * scale,
    );
    assert_eq!(scaled, left_point);
    assert_eq!(scaled.into_affine(), left_affine);

    let normalized = TEProjective::<P>::normalize_batch(&[
        TEProjective::<P>::zero(),
        left_point,
        scaled,
    ]);
    assert_eq!(normalized, [TEAffine::<P>::zero(), left_affine, left_affine]);
    assert!(TEProjective::<P>::normalize_batch(&[]).is_empty());

    let mut right_copy = right_point;
    assert_eq!(&left_point + right_point, expected);
    assert_eq!(&left_point + &right_point, expected);
    assert_eq!(&left_point + &mut right_copy, expected);
    assert_eq!(left_point + &mut right_copy, expected);
    assert_eq!(&left_point - right_point, expected_difference);
    assert_eq!(&left_point - &right_point, expected_difference);
    assert_eq!(&left_point - &mut right_copy, expected_difference);

    let mut assigned = left_point;
    assigned += right_point;
    assert_eq!(assigned, expected);
    assigned -= right_point;
    assert_eq!(assigned, left_point);
    let mut assigned_mut = left_point;
    let mut rhs_mut = right_point;
    assigned_mut += &mut rhs_mut;
    assert_eq!(assigned_mut, expected);
    assigned_mut -= &mut rhs_mut;
    assert_eq!(assigned_mut, left_point);

    let affine_sum: TEProjective<P> =
        [left_affine, right_point.into_affine()].into_iter().sum();
    assert_eq!(affine_sum, expected);
    let affine_ref_sum: TEProjective<P> =
        [&left_affine, &right_point.into_affine()].into_iter().sum();
    assert_eq!(affine_ref_sum, expected);
    let projective_ref_sum: TEProjective<P> =
        [&left_point, &right_point].into_iter().sum();
    assert_eq!(projective_ref_sum, expected);

    assert_eq!(
        left_point * P::ScalarField::from(0u64),
        TEProjective::<P>::zero()
    );
    assert_eq!(left_point * P::ScalarField::from(1u64), left_point);
    assert_eq!(
        left_point * P::ScalarField::from(2u64),
        left_point + left_point
    );
}

macro_rules! sw_properties {
    ($test_name:ident, $config:ty) => {
        proptest! {
            #[test]
            fn $test_name(left in 0u64..32, right in 0u64..32, scale in 1u8..16) {
                check_sw_group::<$config>(left, right, scale);
            }
        }
    };
}

macro_rules! te_properties {
    ($test_name:ident, $config:ty) => {
        proptest! {
            #[test]
            fn $test_name(left in 0u64..32, right in 0u64..32, scale in 1u8..16) {
                check_te_group::<$config>(left, right, scale);
            }
        }
    };
}

sw_properties!(secp256k1_group_properties, Secp256k1Config);
sw_properties!(starknet_group_properties, StarknetCurveConfig);
te_properties!(baby_jubjub_group_properties, BabyJubjubConfig);
te_properties!(bandersnatch_group_properties, BandersnatchConfig);
te_properties!(curve25519_group_properties, Curve25519Config);
te_properties!(jubjub_group_properties, JubjubConfig);

#[test]
fn configured_baby_jubjub_generator_is_in_the_prime_order_subgroup() {
    assert!(
        TEAffine::<BabyJubjubConfig>::generator().is_in_prime_order_subgroup()
    );
}

#[test]
fn checked_short_weierstrass_constructors_accept_generators_and_reject_off_curve_points(
) {
    let generator = SWAffine::<Secp256k1Config>::generator();
    assert_eq!(
        SWAffine::<Secp256k1Config>::new(generator.x, generator.y),
        generator
    );
    let projective = SWProjective::<Secp256k1Config>::from(generator);
    assert_eq!(
        SWProjective::<Secp256k1Config>::new(
            projective.x,
            projective.y,
            projective.z
        ),
        projective,
    );

    assert!(std::panic::catch_unwind(|| {
        SWAffine::<Secp256k1Config>::new(Secp256k1Fq::ZERO, Secp256k1Fq::ZERO)
    })
    .is_err());
}

#[test]
fn checked_twisted_edwards_constructors_accept_generators_and_reject_off_curve_points(
) {
    let generator = TEAffine::<BabyJubjubConfig>::generator();
    assert_eq!(
        TEAffine::<BabyJubjubConfig>::new(generator.x, generator.y),
        generator
    );
    let projective = TEProjective::<BabyJubjubConfig>::from(generator);
    assert_eq!(
        TEProjective::<BabyJubjubConfig>::new(
            projective.x,
            projective.y,
            projective.t,
            projective.z,
        ),
        projective,
    );

    assert!(std::panic::catch_unwind(|| {
        TEAffine::<BabyJubjubConfig>::new(
            BabyJubjubFq::ZERO,
            BabyJubjubFq::ZERO,
        )
    })
    .is_err());
}

#[test]
fn short_weierstrass_zero_z_normalizes_to_infinity() {
    let malformed = SWProjective::<Secp256k1Config>::new_unchecked(
        Secp256k1Fq::from(7u64),
        Secp256k1Fq::from(9u64),
        Secp256k1Fq::ZERO,
    );
    assert_eq!(malformed.into_affine(), SWAffine::<Secp256k1Config>::zero());
    assert_eq!(
        SWProjective::<Secp256k1Config>::normalize_batch(&[malformed]),
        [SWAffine::<Secp256k1Config>::zero()],
    );
}

#[test]
#[should_panic(expected = "projective Z coordinate should not be zero")]
fn twisted_edwards_batch_normalization_rejects_zero_z() {
    let malformed = TEProjective::<BabyJubjubConfig>::new_unchecked(
        BabyJubjubFq::ZERO,
        BabyJubjubFq::ONE,
        BabyJubjubFq::ZERO,
        BabyJubjubFq::ZERO,
    );
    let _ = TEProjective::<BabyJubjubConfig>::normalize_batch(&[malformed]);
}
