//! Deterministic pseudo-random numbers for tests, fuzzing and benchmarks.

/// XorShift128+ with a documented 64-bit state. Not cryptographic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rng {
    s0: u64,
    s1: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        let mut rng = Self {
            s0: seed ^ 0x9e37_79b9_7f4a_7c15,
            s1: seed.rotate_left(27) ^ 0xbf58_476d_1ce4_e5b9,
        };
        if rng.s0 == 0 && rng.s1 == 0 {
            rng.s0 = 1;
        }
        rng
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.s0;
        let y = self.s1;
        self.s0 = y;
        x ^= x << 23;
        self.s1 = x ^ y ^ (x >> 17) ^ (y >> 26);
        self.s1.wrapping_add(y)
    }

    /// Uniform in `[0, 1)`.
    pub fn next_f64(&mut self) -> f64 {
        (self.next_u64() >> 11) as f64 * (1.0 / 9_007_199_254_740_992.0)
    }

    /// Uniform in `[lo, hi)`. Returns `lo` for inverted or non-finite bounds.
    pub fn range(&mut self, lo: f64, hi: f64) -> f64 {
        if !lo.is_finite() || !hi.is_finite() || hi <= lo {
            return lo;
        }
        lo + (hi - lo) * self.next_f64()
    }

    pub fn integer(&mut self, lo: i64, hi: i64) -> i64 {
        if hi <= lo {
            return lo;
        }
        let span = (hi - lo) as u64;
        lo + (self.next_u64() % span) as i64
    }

    pub fn bool(&mut self, probability: f64) -> bool {
        self.next_f64() < probability.clamp(0.0, 1.0)
    }

    pub fn bytes(&mut self, len: usize) -> Vec<u8> {
        let mut out = Vec::with_capacity(len);
        while out.len() < len {
            out.extend_from_slice(&self.next_u64().to_le_bytes());
        }
        out.truncate(len);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deterministic_for_a_seed() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..100 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn different_seeds_differ() {
        let mut a = Rng::new(1);
        let mut b = Rng::new(2);
        assert_ne!(a.next_u64(), b.next_u64());
    }

    #[test]
    fn floats_are_bounded() {
        let mut rng = Rng::new(7);
        for _ in 0..10_000 {
            let v = rng.next_f64();
            assert!((0.0..1.0).contains(&v));
            let r = rng.range(-5.0, 5.0);
            assert!((-5.0..5.0).contains(&r));
        }
    }

    #[test]
    fn bad_ranges_are_safe() {
        let mut rng = Rng::new(9);
        assert_eq!(rng.range(2.0, 1.0), 2.0);
        assert!(rng.range(f64::NAN, 1.0).is_nan());
        assert_eq!(rng.integer(5, 5), 5);
    }
}
