#[test]
fn configured_baby_jubjub_generator_is_in_the_prime_order_subgroup() {
    use super::{
        te::{instance::baby_jubjub::BabyJubjubConfig, Affine},
        AffineRepr,
    };

    assert!(
        Affine::<BabyJubjubConfig>::generator().is_in_prime_order_subgroup()
    );
}
