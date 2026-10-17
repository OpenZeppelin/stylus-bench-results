//! This module contains the Poseidon hash ([whitepaper]) function implemented
//! as a [Sponge Function].
//!
//! Poseidon permutation here follows referenced in [whitepaper] original [rust
//! implementation] with slight improvements.
//!
//! ## Important Usage Notes
//!
//! This interface provides low-level primitives and does not implement padding
//! or domain separation. Users are responsible for:
//! - Padding inputs appropriately
//! - Prepending domain separation tags when needed
//! - Managing absorb/squeeze transitions correctly
//! - Ensuring proper security practices for their specific use case
//!
//! [Sponge function]: https://en.wikipedia.org/wiki/Sponge_function
//! [whitepaper]: https://eprint.iacr.org/2023/323.pdf
//! [rust implementation]: https://github.com/HorizenLabs/poseidon2

pub mod instance;
pub mod params;

use alloc::{boxed::Box, vec, vec::Vec};

use crate::{
    bits::BitIteratorBE, field::prime::PrimeField,
    poseidon2::params::PoseidonParams,
};

/// Determines whether poseidon sponge in absorbing or squeezing state.
/// In squeezing state, sponge can only squeeze elements.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Mode {
    /// Sponge is in absorbing state.
    Absorbing,
    /// Sponge is in squeezing state.
    Squeezing,
}

/// Poseidon2 sponge that can absorb any number of `F` field elements and be
/// squeezed to a finite number of `F` field elements.
///
/// ## Security Notice
///
/// This is a low-level primitive that does not implement padding or domain
/// separation. Users must ensure proper input formatting and security practices
/// for their specific cryptographic protocols.
#[derive(Clone, Debug)]
pub struct Poseidon2<P: PoseidonParams<F>, F: PrimeField> {
    phantom: core::marker::PhantomData<P>,
    state: Box<[F]>,
    mode: Mode,
    index: usize,
}

impl<P: PoseidonParams<F>, F: PrimeField> Default for Poseidon2<P, F> {
    fn default() -> Self {
        Self::new()
    }
}

impl<P: PoseidonParams<F>, F: PrimeField> Poseidon2<P, F> {
    /// Create a new Poseidon sponge.
    ///
    /// # Panics
    ///
    /// Panics if `P::D` is not greater than one or is not coprime to the field
    /// order, because then the S-box is not a permutation.
    #[must_use]
    #[inline]
    pub fn new() -> Self {
        assert!(
            Self::sbox_exponent_is_permutation(),
            "Poseidon S-box exponent must be greater than one and coprime to the field order"
        );

        Self {
            phantom: core::marker::PhantomData,
            state: vec![F::zero(); P::T].into_boxed_slice(),
            mode: Mode::Absorbing,
            // Begin index from `CAPACITY`. Skip capacity elements.
            index: P::CAPACITY,
        }
    }

    /// Check whether raising to `P::D` is a permutation of the field.
    #[inline]
    fn sbox_exponent_is_permutation() -> bool {
        let degree = u16::from(P::D);
        if degree <= 1 {
            return false;
        }

        // x -> x^D is a permutation of F_p exactly when gcd(D, p - 1) = 1.
        // Reduce p modulo D while iterating over its bits, avoiding conversion
        // of the field modulus to a machine-sized integer.
        let modulus_mod_degree =
            F::MODULUS.bit_be_trimmed_iter().fold(0u16, |remainder, bit| {
                (remainder * 2 + u16::from(bit)) % degree
            });
        let mut left = degree;
        let mut right = (modulus_mod_degree + degree - 1) % degree;
        while right != 0 {
            (left, right) = (right, left % right);
        }
        left == 1
    }

    /// Size of poseidon sponge's state.
    #[must_use]
    pub const fn state_size() -> usize {
        P::T
    }

    /// Start index of partial rounds.
    ///
    /// This represents the point where the algorithm transitions from full
    /// rounds to partial rounds in the Poseidon permutation.
    #[must_use]
    const fn partial_round_start() -> usize {
        P::ROUNDS_F / 2
    }

    /// End index of partial rounds (noninclusive).
    ///
    /// This represents the point where the algorithm transitions from partial
    /// rounds back to full rounds in the Poseidon permutation.
    #[must_use]
    const fn partial_round_end() -> usize {
        Self::partial_round_start() + P::ROUNDS_P
    }

    /// Total number of rounds.
    ///
    /// This is the sum of full rounds and partial rounds in the Poseidon
    /// permutation.
    #[must_use]
    const fn rounds() -> usize {
        P::ROUNDS_F + P::ROUNDS_P
    }

    /// Absorb a single element into the sponge.
    ///
    /// Transitions from [`Mode::Absorbing`] to [`Mode::Squeezing`] mode are
    /// unidirectional.
    ///
    /// # Panics
    ///
    /// May panic if absorbing while squeezing.
    #[inline]
    pub fn absorb(&mut self, elem: &F) {
        if let Mode::Squeezing = self.mode {
            panic!("cannot absorb while squeezing");
        }

        if self.index == Self::state_size() {
            self.permute();
            self.index = P::CAPACITY;
        }

        self.state[self.index] += elem;
        self.index += 1;
    }

    /// Absorb batch of elements into the sponge.
    #[inline]
    pub fn absorb_batch(&mut self, elems: &[F]) {
        for elem in elems {
            self.absorb(elem);
        }
    }

    /// Permute elements in the sponge.
    #[inline]
    pub fn permute(&mut self) {
        // Linear layer at the beginning.
        self.matmul_external();

        // Run the first half of the full round.
        for round in 0..Self::partial_round_start() {
            self.external_round(round);
        }

        // Run the partial round.
        for round in Self::partial_round_start()..Self::partial_round_end() {
            self.internal_round(round);
        }

        // Run the second half of the full round.
        for round in Self::partial_round_end()..Self::rounds() {
            self.external_round(round);
        }
    }

    /// Apply external round to the state.
    ///
    /// External rounds apply S-box to all elements of the state vector,
    /// followed by the MDS matrix multiplication.
    #[inline]
    fn external_round(&mut self, round: usize) {
        self.add_rc_external(round);
        self.apply_sbox_external();
        self.matmul_external();
    }

    /// Apply internal round to the state.
    ///
    /// Internal rounds apply S-box only to the first element of the state
    /// vector, followed by the MDS matrix multiplication, which is more
    /// efficient.
    #[inline]
    fn internal_round(&mut self, round: usize) {
        self.add_rc_internal(round);
        self.apply_sbox_internal();
        self.matmul_internal();
    }

    /// Squeeze a single element from the sponge.
    ///
    /// When invoked from [`Mode::Absorbing`] mode, this function triggers a
    /// permutation and transitions to [`Mode::Squeezing`] mode.
    #[inline]
    pub fn squeeze(&mut self) -> F {
        if self.mode == Mode::Absorbing || self.index == Self::state_size() {
            self.permute();
            self.mode = Mode::Squeezing;
            self.index = P::CAPACITY;
        }

        let elem = self.state[self.index];
        self.index += 1;
        elem
    }

    /// Squeeze a batch of elements from the sponge.
    #[inline]
    pub fn squeeze_batch(&mut self, n: usize) -> Vec<F> {
        (0..n).map(|_| self.squeeze()).collect()
    }

    /// Apply sbox to the entire state in the external round.
    ///
    /// This raises each element in the state to the power of D, which is
    /// the S-box degree defined in the Poseidon parameters.
    #[inline]
    fn apply_sbox_external(&mut self) {
        for elem in &mut self.state {
            *elem = elem.pow(P::D);
        }
    }

    /// Apply sbox to the first element in the internal round.
    ///
    /// This applies the S-box (raising to power D) only to the first element of
    /// the state, which is more efficient than applying it to all elements.
    #[inline]
    fn apply_sbox_internal(&mut self) {
        self.state[0] = self.state[0].pow(P::D);
    }

    /// Apply the external MDS matrix `M_E` to the state.
    ///
    /// This function applies the Maximum Distance Separable (MDS) matrix
    /// multiplication to the entire state vector for external rounds of the
    /// Poseidon permutation. The implementation is optimized for different
    /// state sizes.
    #[allow(clippy::needless_range_loop)]
    #[inline(always)]
    fn matmul_external(&mut self) {
        let t = Self::state_size();
        match t {
            2 => {
                // Matrix circ(2, 1)
                let sum = self.state[0] + self.state[1];
                self.state[0] += sum;
                self.state[1] += sum;
            }
            3 => {
                // Matrix circ(2, 1, 1).
                let sum = self.state[0] + self.state[1] + self.state[2];
                self.state[0] += sum;
                self.state[1] += sum;
                self.state[2] += sum;
            }
            4 => {
                self.matmul_m4();
            }
            8 | 12 | 16 | 20 | 24 => {
                self.matmul_m4();

                // Applying second cheap matrix for t > 4.
                let t4 = t / 4;
                let mut stored = [F::zero(); 4];
                for l in 0..4 {
                    stored[l] = self.state[l];
                    for j in 1..t4 {
                        stored[l] += &self.state[4 * j + l];
                    }
                }
                for i in 0..self.state.len() {
                    self.state[i] += &stored[i % 4];
                }
            }
            _ => {
                panic!("not supported state size")
            }
        }
    }

    /// Apply the cheap 4x4 MDS matrix to each 4-element part of the state.
    ///
    /// Optimized matrix multiplication for state sizes that are multiples of 4.
    /// Uses efficient in-place operations instead of constructing the full
    /// matrix.
    #[inline(always)]
    fn matmul_m4(&mut self) {
        let state = &mut self.state;
        let t = Self::state_size();
        let t4 = t / 4;
        for i in 0..t4 {
            let start_index = i * 4;
            let mut t_0 = state[start_index];
            t_0 += &state[start_index + 1];
            let mut t_1 = state[start_index + 2];
            t_1 += &state[start_index + 3];
            let mut t_2 = state[start_index + 1];
            t_2.double_in_place();
            t_2 += &t_1;
            let mut t_3 = state[start_index + 3];
            t_3.double_in_place();
            t_3 += &t_0;
            let mut t_4 = t_1;
            t_4.double_in_place();
            t_4.double_in_place();
            t_4 += &t_3;
            let mut t_5 = t_0;
            t_5.double_in_place();
            t_5.double_in_place();
            t_5 += &t_2;
            let mut t_6 = t_3;
            t_6 += &t_5;
            let mut t_7 = t_2;
            t_7 += &t_4;
            state[start_index] = t_6;
            state[start_index + 1] = t_5;
            state[start_index + 2] = t_7;
            state[start_index + 3] = t_4;
        }
    }

    /// Apply the internal MDS matrix to the state.
    ///
    /// Optimized matrix multiplication for internal rounds.
    #[inline(always)]
    fn matmul_internal(&mut self) {
        let t = Self::state_size();

        match t {
            2 => {
                // [2, 1]
                // [1, 3]
                let sum = self.state[0] + self.state[1];
                self.state[0] += &sum;
                self.state[1].double_in_place();
                self.state[1] += &sum;
            }
            3 => {
                // [2, 1, 1]
                // [1, 2, 1]
                // [1, 1, 3]
                let sum = self.state[0] + self.state[1] + self.state[2];
                self.state[0] += &sum;
                self.state[1] += &sum;
                self.state[2].double_in_place();
                self.state[2] += &sum;
            }
            4 | 8 | 12 | 16 | 20 | 24 => {
                let sum = self.state.iter().sum();

                // Add sum + diag entry * element to each element.
                for i in 0..self.state.len() {
                    self.state[i] *= &P::MAT_INTERNAL_DIAG_M_1[i];
                    self.state[i] += &sum;
                }
            }
            _ => {
                panic!("not supported state size")
            }
        }
    }

    /// Add a round constant to the entire state in external round.
    #[inline]
    fn add_rc_external(&mut self, round: usize) {
        for (a, b) in
            self.state.iter_mut().zip(P::ROUND_CONSTANTS[round].iter())
        {
            *a += b;
        }
    }

    // Add a round constant to the first state element in internal round.
    #[inline]
    fn add_rc_internal(&mut self, round: usize) {
        self.state[0] += P::ROUND_CONSTANTS[round][0];
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        field::instance::FpPallas, poseidon2::instance::pallas::PallasParams,
    };

    struct SquaringPallasParams;

    impl PoseidonParams<FpPallas> for SquaringPallasParams {
        const T: usize = <PallasParams as PoseidonParams<FpPallas>>::T;
        const D: u8 = 2;
        const CAPACITY: usize =
            <PallasParams as PoseidonParams<FpPallas>>::CAPACITY;
        const ROUNDS_F: usize =
            <PallasParams as PoseidonParams<FpPallas>>::ROUNDS_F;
        const ROUNDS_P: usize =
            <PallasParams as PoseidonParams<FpPallas>>::ROUNDS_P;
        const MAT_INTERNAL_DIAG_M_1: &'static [FpPallas] =
            <PallasParams as PoseidonParams<FpPallas>>::MAT_INTERNAL_DIAG_M_1;
        const ROUND_CONSTANTS: &'static [&'static [FpPallas]] =
            <PallasParams as PoseidonParams<FpPallas>>::ROUND_CONSTANTS;
    }

    struct CustomFullRoundsPallasParams<const ROUNDS_F: usize>;

    impl<const ROUNDS_F: usize> PoseidonParams<FpPallas>
        for CustomFullRoundsPallasParams<ROUNDS_F>
    {
        const T: usize = <PallasParams as PoseidonParams<FpPallas>>::T;
        const D: u8 = <PallasParams as PoseidonParams<FpPallas>>::D;
        const CAPACITY: usize =
            <PallasParams as PoseidonParams<FpPallas>>::CAPACITY;
        const ROUNDS_F: usize = ROUNDS_F;
        const ROUNDS_P: usize =
            <PallasParams as PoseidonParams<FpPallas>>::ROUNDS_P;
        const MAT_INTERNAL_DIAG_M_1: &'static [FpPallas] =
            <PallasParams as PoseidonParams<FpPallas>>::MAT_INTERNAL_DIAG_M_1;
        const ROUND_CONSTANTS: &'static [&'static [FpPallas]] =
            <PallasParams as PoseidonParams<FpPallas>>::ROUND_CONSTANTS;
    }

    fn unchecked_sponge() -> Poseidon2<SquaringPallasParams, FpPallas> {
        // Recreate the pre-guard constructor for the collision regression.
        Poseidon2 {
            phantom: core::marker::PhantomData,
            state: alloc::vec![FpPallas::from(0u8); SquaringPallasParams::T]
                .into_boxed_slice(),
            mode: Mode::Absorbing,
            index: SquaringPallasParams::CAPACITY,
        }
    }

    #[test]
    fn invalid_sbox_makes_the_sponge_hash_collide() {
        // For one Poseidon round, choose two rate inputs so the first
        // post-constant state vectors differ by signs in two coordinates.
        // Squaring erases those signs; all remaining rounds therefore match.
        let rc =
            <SquaringPallasParams as PoseidonParams<FpPallas>>::ROUND_CONSTANTS
                [0];
        let zero = FpPallas::from(0u8);
        let two = FpPallas::from(2u8);
        let three = FpPallas::from(3u8);
        let four = FpPallas::from(4u8);
        let five = FpPallas::from(5u8);
        let x = rc[1] - three * rc[0];
        let y = zero;
        let x_collision = five * rc[0] - three * rc[1];
        let y_collision = two * rc[1] - four * rc[0];

        assert_ne!((x, y), (x_collision, y_collision));

        let mut a = unchecked_sponge();
        let mut b = unchecked_sponge();
        a.absorb(&x);
        a.absorb(&y);
        b.absorb(&x_collision);
        b.absorb(&y_collision);

        assert_eq!(a.squeeze(), b.squeeze());
    }

    #[test]
    fn custom_non_permutation_sbox_must_be_rejected() {
        assert!(!Poseidon2::<SquaringPallasParams, FpPallas>::sbox_exponent_is_permutation());
        assert!(std::panic::catch_unwind(
            Poseidon2::<SquaringPallasParams, FpPallas>::new
        )
        .is_err());
    }

    #[test]
    #[should_panic(expected = "Poseidon2 full round count must be even")]
    fn custom_odd_full_round_count_must_be_rejected() {
        let _ = Poseidon2::<CustomFullRoundsPallasParams<7>, FpPallas>::new();
    }

    #[test]
    #[should_panic(expected = "Poseidon2 requires at least 8 full rounds")]
    fn custom_full_round_count_below_security_minimum_must_be_rejected() {
        let _ = Poseidon2::<CustomFullRoundsPallasParams<6>, FpPallas>::new();
    }
}
