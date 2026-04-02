use super::defs::{Accumulator, HIDDEN_SIZE, QA};

// ─── AVX2 (x86_64) ─────────────────────────────────────────────────────────

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
pub unsafe fn add_i16_256(destination: &mut Accumulator, source: &Accumulator) {
    use std::arch::x86_64::*;
    let destination_ptr = destination.data.as_mut_ptr();
    let source_ptr = source.data.as_ptr();
    for offset in (0..HIDDEN_SIZE).step_by(16) {
        let a = _mm256_load_si256(destination_ptr.add(offset) as *const __m256i);
        let b = _mm256_load_si256(source_ptr.add(offset) as *const __m256i);
        _mm256_store_si256(destination_ptr.add(offset) as *mut __m256i, _mm256_add_epi16(a, b));
    }
}

#[cfg(target_arch = "x86_64")]
#[target_feature(enable = "avx2")]
pub unsafe fn sub_i16_256(destination: &mut Accumulator, source: &Accumulator) {
    use std::arch::x86_64::*;
    let destination_ptr = destination.data.as_mut_ptr();
    let source_ptr = source.data.as_ptr();
    for offset in (0..HIDDEN_SIZE).step_by(16) {
        let a = _mm256_load_si256(destination_ptr.add(offset) as *const __m256i);
        let b = _mm256_load_si256(source_ptr.add(offset) as *const __m256i);
        _mm256_store_si256(destination_ptr.add(offset) as *mut __m256i, _mm256_sub_epi16(a, b));
    }
}

#[cfg(target_arch = "x86_64")]
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

// ─── NEON (aarch64) ─────────────────────────────────────────────────────────

#[cfg(target_arch = "aarch64")]
pub fn add_i16_256(destination: &mut Accumulator, source: &Accumulator) {
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
pub fn sub_i16_256(destination: &mut Accumulator, source: &Accumulator) {
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

// ─── Scalar fallback ────────────────────────────────────────────────────────

#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
pub fn add_i16_256(destination: &mut Accumulator, source: &Accumulator) {
    for index in 0..HIDDEN_SIZE {
        destination.data[index] += source.data[index];
    }
}

#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
pub fn sub_i16_256(destination: &mut Accumulator, source: &Accumulator) {
    for index in 0..HIDDEN_SIZE {
        destination.data[index] -= source.data[index];
    }
}

#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
pub fn screlu_activate(accumulator: &Accumulator, output: &mut [i32], output_offset: usize) {
    for index in 0..HIDDEN_SIZE {
        let clamped = accumulator.data[index].clamp(0, QA as i16) as i32;
        output[output_offset + index] = clamped * clamped;
    }
}
