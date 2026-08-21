use super::defs::{Accumulator, HIDDEN_SIZE, QA};

// ─── Scalar ─────────────────────────────────────────────────────────────────
//
// The fallback for x86_64 CPUs without AVX2, the only implementation on
// architectures with no hand-written kernel, and the reference the SIMD kernels
// are tested against. aarch64 release builds never call it, hence the allow.

#[allow(dead_code)]
mod scalar {
    use super::{Accumulator, HIDDEN_SIZE, QA};

    pub fn add_i16(destination: &mut Accumulator, source: &Accumulator) {
        for index in 0..HIDDEN_SIZE {
            destination.data[index] += source.data[index];
        }
    }

    pub fn sub_i16(destination: &mut Accumulator, source: &Accumulator) {
        for index in 0..HIDDEN_SIZE {
            destination.data[index] -= source.data[index];
        }
    }

    pub fn screlu_activate(accumulator: &Accumulator, output: &mut [i32], output_offset: usize) {
        for index in 0..HIDDEN_SIZE {
            let clamped = accumulator.data[index].clamp(0, QA as i16) as i32;
            output[output_offset + index] = clamped * clamped;
        }
    }
}

// ─── AVX2 (x86_64) ─────────────────────────────────────────────────────────

#[cfg(target_arch = "x86_64")]
mod avx2 {
    use super::super::defs::{Accumulator, HIDDEN_SIZE, QA};

    /// # Safety
    /// The caller must have verified that the CPU supports AVX2.
    #[target_feature(enable = "avx2")]
    pub unsafe fn add_i16(destination: &mut Accumulator, source: &Accumulator) {
        use std::arch::x86_64::*;
        let destination_ptr = destination.data.as_mut_ptr();
        let source_ptr = source.data.as_ptr();
        for offset in (0..HIDDEN_SIZE).step_by(16) {
            let a = _mm256_load_si256(destination_ptr.add(offset) as *const __m256i);
            let b = _mm256_load_si256(source_ptr.add(offset) as *const __m256i);
            _mm256_store_si256(destination_ptr.add(offset) as *mut __m256i, _mm256_add_epi16(a, b));
        }
    }

    /// # Safety
    /// The caller must have verified that the CPU supports AVX2.
    #[target_feature(enable = "avx2")]
    pub unsafe fn sub_i16(destination: &mut Accumulator, source: &Accumulator) {
        use std::arch::x86_64::*;
        let destination_ptr = destination.data.as_mut_ptr();
        let source_ptr = source.data.as_ptr();
        for offset in (0..HIDDEN_SIZE).step_by(16) {
            let a = _mm256_load_si256(destination_ptr.add(offset) as *const __m256i);
            let b = _mm256_load_si256(source_ptr.add(offset) as *const __m256i);
            _mm256_store_si256(destination_ptr.add(offset) as *mut __m256i, _mm256_sub_epi16(a, b));
        }
    }

    /// # Safety
    /// The caller must have verified that the CPU supports AVX2, and `output`
    /// must hold at least `output_offset + HIDDEN_SIZE` elements.
    #[target_feature(enable = "avx2")]
    pub unsafe fn screlu_activate(accumulator: &Accumulator, output: &mut [i32], output_offset: usize) {
        use std::arch::x86_64::*;
        let zero = _mm256_setzero_si256();
        let qa_vector = _mm256_set1_epi16(QA as i16);
        let accumulator_ptr = accumulator.data.as_ptr();
        let output_ptr = output.as_mut_ptr().add(output_offset);
        for offset in (0..HIDDEN_SIZE).step_by(16) {
            let values = _mm256_load_si256(accumulator_ptr.add(offset) as *const __m256i);
            let clamped = _mm256_min_epi16(_mm256_max_epi16(values, zero), qa_vector);
            // Unpack low 8 i16 → 8 i32, square them
            let low_half = _mm256_cvtepi16_epi32(_mm256_castsi256_si128(clamped));
            let high_half = _mm256_cvtepi16_epi32(_mm256_extracti128_si256(clamped, 1));
            _mm256_storeu_si256(
                output_ptr.add(offset) as *mut __m256i,
                _mm256_mullo_epi32(low_half, low_half),
            );
            _mm256_storeu_si256(
                output_ptr.add(offset + 8) as *mut __m256i,
                _mm256_mullo_epi32(high_half, high_half),
            );
        }
    }
}

/// Whether the AVX2 kernels are safe to call on this CPU.
///
/// A native build (`-C target-cpu=native`, the default in `.cargo/config.toml`)
/// answers this at compile time and the branch folds away. The distribution
/// build is generic x86-64, so it has to ask the CPU — `is_x86_feature_detected!`
/// caches the answer after the first call.
#[cfg(target_arch = "x86_64")]
#[inline(always)]
fn has_avx2() -> bool {
    cfg!(target_feature = "avx2") || std::is_x86_feature_detected!("avx2")
}

#[cfg(target_arch = "x86_64")]
#[inline]
pub fn add_i16(destination: &mut Accumulator, source: &Accumulator) {
    if has_avx2() {
        unsafe { avx2::add_i16(destination, source) }
    } else {
        scalar::add_i16(destination, source);
    }
}

#[cfg(target_arch = "x86_64")]
#[inline]
pub fn sub_i16(destination: &mut Accumulator, source: &Accumulator) {
    if has_avx2() {
        unsafe { avx2::sub_i16(destination, source) }
    } else {
        scalar::sub_i16(destination, source);
    }
}

#[cfg(target_arch = "x86_64")]
#[inline]
pub fn screlu_activate(accumulator: &Accumulator, output: &mut [i32], output_offset: usize) {
    assert!(output.len() >= output_offset + HIDDEN_SIZE);
    if has_avx2() {
        unsafe { avx2::screlu_activate(accumulator, output, output_offset) }
    } else {
        scalar::screlu_activate(accumulator, output, output_offset);
    }
}

// ─── NEON (aarch64) ─────────────────────────────────────────────────────────
//
// NEON is baseline on aarch64, so no runtime detection is needed.

#[cfg(target_arch = "aarch64")]
pub fn add_i16(destination: &mut Accumulator, source: &Accumulator) {
    use std::arch::aarch64::*;
    let destination_ptr = destination.data.as_mut_ptr();
    let source_ptr = source.data.as_ptr();
    for offset in (0..HIDDEN_SIZE).step_by(8) {
        unsafe {
            let a = vld1q_s16(destination_ptr.add(offset));
            let b = vld1q_s16(source_ptr.add(offset));
            vst1q_s16(destination_ptr.add(offset), vaddq_s16(a, b));
        }
    }
}

#[cfg(target_arch = "aarch64")]
pub fn sub_i16(destination: &mut Accumulator, source: &Accumulator) {
    use std::arch::aarch64::*;
    let destination_ptr = destination.data.as_mut_ptr();
    let source_ptr = source.data.as_ptr();
    for offset in (0..HIDDEN_SIZE).step_by(8) {
        unsafe {
            let a = vld1q_s16(destination_ptr.add(offset));
            let b = vld1q_s16(source_ptr.add(offset));
            vst1q_s16(destination_ptr.add(offset), vsubq_s16(a, b));
        }
    }
}

#[cfg(target_arch = "aarch64")]
pub fn screlu_activate(accumulator: &Accumulator, output: &mut [i32], output_offset: usize) {
    use std::arch::aarch64::*;
    assert!(output.len() >= output_offset + HIDDEN_SIZE);
    let zero = unsafe { vdupq_n_s16(0) };
    let qa_vector = unsafe { vdupq_n_s16(QA as i16) };
    let accumulator_ptr = accumulator.data.as_ptr();
    let output_ptr = output.as_mut_ptr();
    for offset in (0..HIDDEN_SIZE).step_by(8) {
        unsafe {
            let values = vld1q_s16(accumulator_ptr.add(offset));
            let clamped = vminq_s16(vmaxq_s16(values, zero), qa_vector);
            // Widen low/high halves to i32, then square
            let low_half = vmovl_s16(vget_low_s16(clamped));
            let high_half = vmovl_s16(vget_high_s16(clamped));
            vst1q_s32(output_ptr.add(output_offset + offset), vmulq_s32(low_half, low_half));
            vst1q_s32(
                output_ptr.add(output_offset + offset + 4),
                vmulq_s32(high_half, high_half),
            );
        }
    }
}

// ─── Other architectures ────────────────────────────────────────────────────

#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
pub fn add_i16(destination: &mut Accumulator, source: &Accumulator) {
    scalar::add_i16(destination, source);
}

#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
pub fn sub_i16(destination: &mut Accumulator, source: &Accumulator) {
    scalar::sub_i16(destination, source);
}

#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
pub fn screlu_activate(accumulator: &Accumulator, output: &mut [i32], output_offset: usize) {
    scalar::screlu_activate(accumulator, output, output_offset);
}

#[cfg(test)]
mod test {
    use super::*;

    fn ramp(start: i16) -> Accumulator {
        let mut accumulator = Accumulator::zeroed();
        for index in 0..HIDDEN_SIZE {
            accumulator.data[index] = start.wrapping_add((index % 509) as i16);
        }
        accumulator
    }

    #[test]
    fn add_matches_scalar() {
        let source = ramp(3);
        let mut simd_result = ramp(-100);
        let mut scalar_result = simd_result;

        add_i16(&mut simd_result, &source);
        scalar::add_i16(&mut scalar_result, &source);

        assert_eq!(simd_result.data, scalar_result.data);
    }

    #[test]
    fn sub_matches_scalar() {
        let source = ramp(3);
        let mut simd_result = ramp(-100);
        let mut scalar_result = simd_result;

        sub_i16(&mut simd_result, &source);
        scalar::sub_i16(&mut scalar_result, &source);

        assert_eq!(simd_result.data, scalar_result.data);
    }

    #[test]
    fn screlu_matches_scalar() {
        // Straddle the clamp on both ends: negatives go to 0, values above QA saturate.
        let accumulator = ramp(-200);
        let mut simd_result = vec![0i32; HIDDEN_SIZE * 2];
        let mut scalar_result = vec![0i32; HIDDEN_SIZE * 2];

        screlu_activate(&accumulator, &mut simd_result, HIDDEN_SIZE);
        scalar::screlu_activate(&accumulator, &mut scalar_result, HIDDEN_SIZE);

        assert_eq!(simd_result, scalar_result);
    }
}
