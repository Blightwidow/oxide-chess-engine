/* Minimal arm64 stand-in for the x86 <immintrin.h> that Stash includes when
 * USE_POPCNT is defined. Only the prefetch intrinsic is actually referenced;
 * popcount goes through __builtin_popcountll. */
#ifndef OXID_IMMINTRIN_SHIM_H
#define OXID_IMMINTRIN_SHIM_H

#define _MM_HINT_T0 3

__attribute__((unused)) static inline void _mm_prefetch(const void *pointer, int hint)
{
    (void)hint;
    __builtin_prefetch(pointer);
}

#endif
