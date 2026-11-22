//! A tiny seeded random number generator (PCG32, "XSH-RR" variant).
//!
//! We write it ourselves (about 20 lines) instead of using the `rand` crate
//! because the rules must give the SAME numbers on every machine, forever.
//! A library update must never silently change old replays.

use serde::{Deserialize, Serialize};

const MULTIPLIER: u64 = 6364136223846793005;

/// Fixed "stream" constant used by [`Pcg32State::new`].
///
/// NEVER change this value: doing so changes every shuffle and breaks every
/// saved replay.
pub const DEFAULT_STREAM: u64 = 0xda3e39cb94b95bdb;

/// The complete state of the generator. Both fields are saved, so a game
/// saved mid-way continues with exactly the same random numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pcg32State {
    /// Current internal state.
    pub state: u64,
    /// Stream increment (always odd).
    pub inc: u64,
}

impl Pcg32State {
    /// Create a generator from a game seed, using the fixed stream.
    pub fn new(seed: u64) -> Self {
        Self::with_stream(seed, DEFAULT_STREAM)
    }

    /// Create a generator from an initial state and a stream selector.
    /// Matches the reference PCG32 `srandom` function.
    pub fn with_stream(init_state: u64, init_seq: u64) -> Self {
        let mut rng = Self { state: 0, inc: (init_seq << 1) | 1 };
        rng.next_u32();
        rng.state = rng.state.wrapping_add(init_state);
        rng.next_u32();
        rng
    }

    /// Next 32 random bits.
    pub fn next_u32(&mut self) -> u32 {
        let old = self.state;
        self.state = old.wrapping_mul(MULTIPLIER).wrapping_add(self.inc);
        let xorshifted = (((old >> 18) ^ old) >> 27) as u32;
        let rot = (old >> 59) as u32;
        xorshifted.rotate_right(rot)
    }

    /// A random number in `0..bound` with no modulo bias.
    /// Returns 0 if `bound` is 0.
    pub fn below(&mut self, bound: u32) -> u32 {
        if bound == 0 {
            return 0;
        }
        // Reject the few low values that would make some results likelier.
        let threshold = bound.wrapping_neg() % bound;
        loop {
            let r = self.next_u32();
            if r >= threshold {
                return r % bound;
            }
        }
    }

    /// Fisher-Yates shuffle. Same state + same input = same output.
    pub fn shuffle<T>(&mut self, items: &mut [T]) {
        for i in (1..items.len()).rev() {
            let j = self.below(i as u32 + 1) as usize;
            items.swap(i, j);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_reference_pcg32_vector() {
        // Official PCG32 demo output for srandom(42, 54).
        let mut rng = Pcg32State::with_stream(42, 54);
        let expected = [0xa15c02b7u32, 0x7b47f409, 0xba1d3330, 0x83d2f293, 0xbfa4784b, 0xcbed606e];
        for want in expected {
            assert_eq!(rng.next_u32(), want);
        }
    }

    #[test]
    fn below_stays_in_range() {
        let mut rng = Pcg32State::new(1);
        for _ in 0..1000 {
            assert!(rng.below(7) < 7);
        }
        assert_eq!(rng.below(0), 0);
    }

    #[test]
    fn shuffle_is_deterministic_and_keeps_all_items() {
        let mut a: Vec<u32> = (0..20).collect();
        let mut b = a.clone();
        Pcg32State::new(99).shuffle(&mut a);
        Pcg32State::new(99).shuffle(&mut b);
        assert_eq!(a, b);
        let mut sorted = a.clone();
        sorted.sort();
        assert_eq!(sorted, (0..20).collect::<Vec<u32>>());
        assert_ne!(a, (0..20).collect::<Vec<u32>>());
    }
}
