# Elo Estimation

Oxide doesn't have an official CCRL rating. To estimate its strength, we run SPRT matches against versions of [Stash](https://github.com/mhouppin/stash-bot), a well-established engine with CCRL-rated releases.

## Method

1. Pick a Stash version close to Oxide's expected strength.
2. Run an SPRT test using [fastchess](https://github.com/Disservin/fastchess):
   ```bash
   ./bin/fastchess \
       -engine cmd=./target/release/chessbot name=oxide \
       -engine cmd=./stash/stash-vXX name=stash-vXX \
       -openings file=8moves_v3.pgn format=pgn order=random \
       -each tc=8+0.08 \
       -rounds 5000 -repeat \
       -concurrency 8 \
       -recover \
       -sprt elo0=0 elo1=10 alpha=0.05 beta=0.1
   ```
3. If Oxide wins the SPRT (H1 accepted), it is likely stronger than that Stash version. Test upward. If it loses (H0 accepted), test downward. The Elo estimate is bracketed by the highest version Oxide beats and the lowest it loses to.

## Elo Table

| Oxide Version | Estimated Elo | Notes |
|---------------|---------------|-------|
| v1.0.0 | ~2400 | SPRT places it -20 elo against Stash v20 (2509) |
| v0.2.0 | ~1900 | Barely beats Stash v12 (1886) |

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
