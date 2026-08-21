# Oxid' Chess Engine — TODO / Roadmap (2765 → 2850+ Elo)

Current strength: ~2765 Elo (see `docs/elo.md` for the anchor matches).

## Next up

- [ ] **NNUE output buckets** (+10-30 Elo) — `src/nnue/`, `training/`
  - Select the output head by material count (typically 8 buckets)
  - Requires matching bucket support in the trainer and a net format bump (v3 → v4)
  - The 256 → 384 hidden-size half of this item shipped in v2.0.0

## Phase A — Low-hanging fruit ✓

- [x] **Continuation history** — shipped in v1.3.0
- [x] **Capture history** — shipped in v1.3.0
- [x] **Bad captures stage** — `STAGE_BAD_CAPTURES` in `src/search/move_picker.rs`

## Phase B — Not implemented

Both items were checked off and documented in `docs/search.md` before any code existed. The docs have
been corrected; the work is still open.

- [ ] **Multi-bucket TT** (+30-50 Elo) — `src/evaluate/transposition.rs`
  - Today: one `Entry` per index, full u64 key, unpacked `HashData`, `key % size` indexing
  - Wanted: 3 entries per bucket, 32-byte aligned; 16-bit verification key, u16 move, packed
    generation + node type; power-of-2 bitmask indexing
  - Replacement: always-replace shallowest/oldest, keep one depth-preferred slot
  - TT prefetch before `do_move`

- [ ] **PV tracking** (+5 Elo indirect) — `src/search.rs`
  - Today: only the best root move is kept, so UCI `pv` carries a single move and the ponder move
    comes from a TT probe
  - Wanted: `pv_table: [[Move; MAX_PLY]; MAX_PLY]` + `pv_length`, child PV copied on alpha
    improvement, full PV in info lines, `score mate N` formatting

## Phase C ✓

- [x] **SIMD for NNUE** — `src/nnue/simd.rs`
  - NEON (aarch64) + AVX2 (x86_64) for accumulator ops and SCReLU
  - AVX2 selected by runtime CPU detection; scalar fallback doubles as the test reference
  - Aligned `Accumulator` struct

- ~~**Singular extension tuning**~~ — canceled
  - Double extensions: SPRT failed at -8 Elo
  - Negative extensions: caused regression
  - Only the depth 10 → 8 threshold change was kept
  - May revisit with a different approach later

- ~~**Aspiration window tuning**~~ — canceled
  - SPRT failed badly (-76 Elo, massive timeouts)
  - Re-searching all root moves on fail too expensive
  - Window stays at +/-25 cp with additive +100 widening

## Phase D ✓

- [x] **Time management improvements** — `src/time.rs`, shipped in #5, SPRT +8.5 Elo
  - Node TM, score stability, eval complexity

## Phase E — Big features

- [x] **Self-play datagen pipeline** — shipped in #7
  - `datagen` UCI command: self-play games → plain text → `.binpack`
  - Compatible with the bullet sfbinpack loader in `training/src/main.rs`

- [x] **NNUE hidden size 256 → 384** — shipped in v2.0.0 as net v3 (`nn-b9f535fc9a86`)
  - Trained on Stockfish `test80-2023` binpacks, not self-play data
  - Output buckets deferred — see "Next up"

- [x] **Endgame tablebases** — `src/tablebase.rs`
  - Root DTZ probe, in-search WDL probe, `SyzygyPath` UCI option
  - Not available in the `oxide-aarch64-linux` artifact (pyrrhic-rs does not build for that target)

- [x] **Opening book support** — `src/book.rs`, `src/uci.rs`
  - Polyglot `.bin` parsing, separate 781-key Polyglot hash, weighted random selection
  - `BookFile` UCI option; falls through to search when out of book
  - `OwnBook` option was never added — the book is enabled by setting `BookFile`

---

## Verification checklist (each change)

1. `cargo clippy && cargo test`
2. NPS comparison at depth 13 (see CLAUDE.md bench commands)
3. SPRT: `./bin/fastchess` tc=8+0.08, elo0=0 elo1=5, 15K rounds minimum
4. Cross-target check before tagging: `cargo check --target x86_64-unknown-linux-gnu` with
   `RUSTFLAGS=""` (the release workflow only runs on tags)
5. NNUE changes: rank candidates with fixed-game matches, then SPRT the winner against the champion
   (see the net selection note in CLAUDE.md — bullet no longer reports validation loss)

---

## Unresolved questions

1. Training data: use Stockfish or Lc0 data — no strong preference at the current strength level
2. ~~SMP thread count~~ — SMP scrapped for now (overcomplex for low return)
3. ~~SMP vs NNUE priority~~ — resolved: NNUE improvements next
4. No DFRC/Chess960 support planned
5. No specific Elo target or tournament — exploratory development
