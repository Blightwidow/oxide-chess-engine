# Evaluation

Evaluation is NNUE-only. `NnueEval::evaluate` (`src/nnue/mod.rs`) returns a centipawn score from the
perspective of the side to move; `src/search.rs:263` is its only caller.

There is no handcrafted evaluation. Earlier versions carried a tapered eval with piece-square tables,
material values and pawn-structure terms as a fallback; it was removed once the net became the primary
eval. `src/evaluate.rs` keeps the name but now only owns the transposition table
(`src/evaluate/transposition.rs`).

## Architecture

```
(8 buckets × 768) inputs → [384] accumulator (per perspective) → SCReLU
[768] concatenated → [32] hidden → SCReLU → [1] output → scale to centipawns
```

Constants live in `src/nnue/defs.rs`:

| Constant | Value | Meaning |
|----------|-------|---------|
| `FEATURE_SIZE` | 768 | 2 colors × 6 piece types × 64 squares |
| `NUM_BUCKETS` | 8 | King buckets (one per rank) |
| `HIDDEN_SIZE` | 384 | Accumulator width per perspective |
| `L1_SIZE` | 32 | Hidden layer width |
| `QA` | 255 | Accumulator quantization scale |
| `QB` | 64 | L1 weight quantization scale |
| `SCALE` | 400 | Output scale to centipawns |

- **Input features**: 6144 = 8 king buckets × 768.
- **King bucketing**: each perspective uses separate feature transformer weights depending on its own
  king's position. Kings are bucketed by rank (0–7) with horizontal mirroring — files e-h map to d-a,
  so the network only learns queen-side king positions and mirrors for king-side.
- **Horizontal mirroring**: when the perspective's king is on files e-h, all piece squares are flipped
  horizontally (`sq ^ 7`) to normalize to the queen-side half.
- **Bucket change refresh**: when a king move changes the bucket or the mirror state, the moving side's
  accumulator is recomputed from scratch. The opponent's accumulator is updated incrementally as usual.
- **Perspective**: white and black accumulators are computed separately. For black, colors are swapped
  and squares vertically flipped.
- **Quantization**: accumulator activations clipped to `[0, QA]`, output scaled by `SCALE / (QA × QB)`.
- **Arithmetic**: pure integer (i16/i32), no floating point.
- **SIMD**: accumulator updates and SCReLU use platform-specific kernels (`src/nnue/simd.rs`) — NEON on
  aarch64, AVX2 on x86_64, scalar fallback elsewhere. Accumulators are 32-byte aligned for AVX2
  load/store.

## Network File Format (v3)

Binary `.nnue` file with header:

- Magic: `OXNN` (4 bytes)
- Version: `3` (u32 LE)
- Num buckets, feature size, hidden size, L1 size (4 × u32 LE)
- Weights and biases as i16 little-endian

The default net is embedded at compile time via `include_bytes!`. A different net can be loaded at
runtime with `setoption name EvalFile value <path>` — useful for SPRT-testing candidates without
recompiling.

## Load Failures

`Network::from_bytes` returns `None` on a bad magic, a version other than `NET_VERSION`, or truncated
data. The two call sites handle that differently:

- **Embedded net incompatible** — the engine falls back to `NnueEval::empty()`, a network with all-zero
  weights. Every position then evaluates to 0, so the engine plays on search alone. This is a loud
  failure by design: it never reinterprets v2 bytes as a v3 net.
- **`EvalFile` load fails** — `Search::load_nnue` keeps the currently loaded net and leaves the search
  untouched.

## Training

Nets are trained with [bullet](https://github.com/jw1912/bullet) (Rust NNUE trainer). Upstream's
backends are `cuda`, `rocm` and `metal` — there is no CPU backend, so `--features metal` is the
supported path on Apple Silicon. See `training/README.md` for full instructions and
`CLAUDE.md` for dataset sizing and backend notes.

> **Note (2026-04):** A PyTorch-based trainer was attempted but abandoned — all nets were consistently
> 300+ Elo weaker than bullet-trained equivalents despite identical architecture. Root cause was
> training quality (LR schedule convergence), not a code bug.
