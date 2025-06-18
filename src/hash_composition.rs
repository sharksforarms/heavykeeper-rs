use std::hash::{BuildHasher, Hash};

/// Generates initial hashes and provides methods for hash composition
pub(crate) struct HashComposer {
    h1: u64,
    h2: u64,
    fingerprint: u64,
}

impl HashComposer {
    /// Creates a new HashComposer from a value using the provided hasher
    #[inline]
    pub fn new<T: Hash, S: BuildHasher>(hasher: &S, value: &T) -> Self {
        let h1 = hasher.hash_one(value);
        let h2 = h1.wrapping_shr(32).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);

        Self {
            h1,
            h2,
            fingerprint: h1,
        }
    }

    /// Gets the fingerprint for bucket matching
    #[inline]
    pub fn fingerprint(&self) -> u64 {
        self.fingerprint
    }

    /// Composes the next hash and returns bucket index
    #[inline]
    pub fn next_bucket(&mut self, width: u64, depth: usize) -> usize {
        if depth > 0 {
            self.h1 = self.h1.wrapping_add(self.h2).rotate_left(5);
        }
        (self.h1 % width) as usize
    }
}
