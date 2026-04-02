# Elo Estimation

Oxid' doesn't have an official CCRL rating. To estimate its strength, we run SPRT matches against versions of [Stash](https://github.com/mhouppin/stash-bot), a well-established engine with CCRL-rated releases.

## Method

1. Pick a Stash version close to Oxid''s expected strength.
2. Run an SPRT test using [fastchess](https://github.com/Disservin/fastchess):
   ```bash
   ./bin/fastchess \
       -engine cmd=./target/release/chessbot name=oxid \
       -engine cmd=./stash/stash-vXX name=stash-vXX \
       -openings file=8moves_v3.pgn format=pgn order=random \
       -each tc=8+0.08 \
       -rounds 5000 -repeat \
       -concurrency 8 \
       -recover \
       -sprt elo0=0 elo1=10 alpha=0.05 beta=0.1
   ```
3. If Oxid' wins the SPRT (H1 accepted), it is likely stronger than that Stash version. Test upward. If it loses (H0 accepted), test downward. The Elo estimate is bracketed by the highest version Oxid' beats and the lowest it loses to.

## Elo Table

| Oxid' Version | Estimated Elo | Notes |
|---------------|---------------|-------|
| v1.1.1 | ~2600 | +46 vs Stash v20 (2509), -31 vs Stash v21 (2714) |
| v1.1.0 | ~2401 | Bucketed NNUE, ~+1 Elo over v1.0.1 |
| v1.0.1 | ~2400 | History malus, TT age replacement, +19.77 Elo over v1.0.0 |
| v1.0.0 | ~2400 | SPRT places it -20 elo against Stash v20 (2509) |
| v0.2.0 | ~1900 | Barely beats Stash v12 (1886) |

## ERET 15s (Quick Elo Estimation)

The [Eigenmann Rapid Engine Test](https://www.chessprogramming.org/Eigenmann_Rapid_Engine_Test) (ERET) is a 111-position EPD suite where the engine has a fixed time (typically 15 seconds) per position to find the best move. The number of correct answers correlates with engine strength, making it a fast single-machine proxy for Elo without needing a full SPRT match.

### Running ERET

```bash
cargo build -r
printf "eret 15\nquit\n" | ./target/release/oxid
```

The argument is time per position in seconds. A full run at 15s takes ~28 minutes. When using 15s, the engine prints an estimated Elo based on a polynomial regression fitted to the reference data.

### Reference Scores (15s per position)

| Engine | Elo | Score |
|--------|-----|-------|
| Stockfish 9 | 3425 | 77/111 |
| Komodo 12 | 3376 | 75/111 |
| Booot 6.3 | 3240 | 57/111 |
| BlackMamba 2.0 | 3091 | 34/111 |
| Alfil 13.1 | 2748 | 17/111 |
| Clueless 1.4 | 1840 | 11/111 |

Use this table to interpolate Oxid''s approximate Elo from its ERET score. The relationship is roughly linear in the 2000–3400 range.

## Stash Reference Ratings

Stash blitz ratings from CCRL (entries marked `*` are estimates, not official CCRL ratings):

| Version | Elo |
|---------|-----|
| v36 | 3399 |
| v35 | 3358 |
| v34 | 3328 |
| v33 | 3286 |
| v32 | 3252 |
| v31 | 3220 |
| v30 | 3166 |
| v29 | 3137 |
| v28 | 3092 |
| v27 | 3057 |
| v26 | 3000\* |
| v25 | 2937 |
| v24 | 2880\* |
| v23 | 2830\* |
| v22 | 2770\* |
| v21 | 2714 |
| v20 | 2509 |
| v19 | 2473 |
| v18 | 2390\* |
| v17 | 2298 |
| v16 | 2220\* |
| v15 | 2140\* |
| v14 | 2060 |
| v13 | 1972 |
| v12 | 1886 |
| v11 | 1690 |
| v10 | 1620\* |
| v9 | 1275 |
| v8 | 1090\* |
