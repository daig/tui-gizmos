//! Deterministic noise. The same seed and position always give the same
//! value, so a frame re-rendered with the same seed is identical, and a new
//! seed re-deals every mark.

/// SplitMix64's finaliser: scrambles all 64 bits.
const fn mix(mut z: u64) -> u64 {
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// 64 random bits for position `(x, y)` on noise layer `layer`.
pub(crate) const fn hash(seed: u64, x: u32, y: u32, layer: u32) -> u64 {
    const STEP: u64 = 0x9E37_79B9_7F4A_7C15;
    let z = mix(seed.wrapping_add(STEP) ^ x as u64);
    let z = mix(z.wrapping_add(STEP) ^ y as u64);
    mix(z.wrapping_add(STEP) ^ layer as u64)
}

/// Three independent uniform values in `[0, 1)` from one hash.
pub(crate) fn uniforms(bits: u64) -> [f32; 3] {
    const MASK: u64 = (1 << 21) - 1;
    let unit = |b: u64| (b & MASK) as f32 / (MASK + 1) as f32;
    [unit(bits), unit(bits >> 21), unit(bits >> 42)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uniforms_are_uniform() {
        let n = 20_000;
        let mut sums = [0f64; 3];
        let mut below = [0u32; 3];
        for i in 0..n {
            for (k, u) in uniforms(hash(7, i % 211, i / 211, 0)).into_iter().enumerate() {
                assert!((0.0..1.0).contains(&u));
                sums[k] += f64::from(u);
                below[k] += u32::from(u < 0.1);
            }
        }
        for k in 0..3 {
            assert!((sums[k] / f64::from(n) - 0.5).abs() < 0.01, "{sums:?}");
            assert!((f64::from(below[k]) / f64::from(n) - 0.1).abs() < 0.01, "{below:?}");
        }
    }

    #[test]
    fn seed_and_layer_decorrelate() {
        assert_ne!(hash(1, 3, 4, 0), hash(2, 3, 4, 0));
        assert_ne!(hash(1, 3, 4, 0), hash(1, 3, 4, 1));
        assert_ne!(hash(1, 3, 4, 0), hash(1, 4, 3, 0));
    }
}
