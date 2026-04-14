# Oxid' Chess Engine — TODO / Roadmap (2625 → 2850+ Elo)

## Phase A — Low-hanging fruit (~+50 Elo) ✓

- [x] **Continuation history** (+30-40 Elo) — shipped in v1.3.0
- [x] **Capture history** (+10-15 Elo) — shipped in v1.3.0
- [x] **Bad captures stage** (+5-10 Elo) — implemented in `src/search/move_picker.rs`

## Phase B (~+40 Elo cumulative) ✓

- [x] **Multi-bucket TT** (+30-50 Elo) — `src/evaluate/transposition.rs`
  - 3 entries per bucket (≤48 bytes, cache-line friendly)
  - Tighter packing: 16-bit key, u16 move, depth+gen+node_type packed
  - Replacement: always-replace shallowest/oldest; keep one depth-preferred slot
  - TT prefetch (`_mm_prefetch`) before `do_move`

- [x] **PV tracking** (+5 Elo indirect) — `src/search.rs`
  - Add `pv_table: [[Move; MAX_PLY]; MAX_PLY]` and `pv_length`
  - Copy child PV on alpha improvement
  - Print full PV in UCI info output

## Phase C (~+10 Elo realized) ✓

- [x] **SIMD for NNUE** (+5-10 Elo via NPS) — `src/nnue/`
  - NEON (aarch64) + AVX2 (x86_64) for accumulator ops and SCReLU forward pass
  - Aligned `Accumulator` struct, scalar fallback for other architectures

- ~~**Singular extension tuning**~~ — canceled
  - Double extensions: SPRT failed at -8 Elo
  - Negative extensions: caused regression
  - May revisit with different approach later

- ~~**Aspiration window tuning**~~ — canceled
  - SPRT failed badly (-76 Elo, massive timeouts)
  - Re-searching all root moves on fail too expensive

## Phase D (~+15 Elo cumulative) ✓

- [x] **Time management improvements** (+10-15 Elo) — `src/time.rs`, shipped in #5
  - Node TM: stop early if best move has >90% root nodes, extend if <50%
  - Score stability: extend on score drops between iterations
  - Eval complexity: use root move score spread to estimate difficulty

## Phase E — Big features (~+80-130 Elo)

- [x] **Self-play datagen pipeline** — shipped in #7
  - `datagen` UCI command: self-play games → plain text → `.binpack`
  - Params: depth, num_games, output_path, 8 random opening plies
  - Record (FEN, move, score, ply, WDL) per position
  - Compatible with bullet sfbinpack loader in `training/src/main.rs`

- [ ] **Improved NNUE architecture** (+30-80 Elo) — `src/nnue/`, `training/`
  - Increase HIDDEN_SIZE to 384 or 512
  - Requires SIMD (Phase C) to keep NPS acceptable
  - Train on 200M+ self-play positions
  - Consider output buckets (material-based eval head selection)

- [x] **End-game base tables / EGTB** (+10-15 Elo) — `src/tablebase.rs`, `src/search.rs`
  - pyrrhic-rs (pure Rust Syzygy wrapper)
  - Root probe for DTZ (best winning move)
  - In-search WDL probe for score adjustment
  - `SyzygyPath` UCI option

- [x] **Opening book support** — `src/book.rs`, `src/uci.rs`
  - Parse Polyglot `.bin` format (binary search, big-endian, 16-byte entries)
  - Separate Polyglot Zobrist hash (781-key table, EP filtering)
  - Weighted random move selection with legal move validation
  - Add `OwnBook` and `BookFile` UCI options
  - Fall through to engine search when out of book

---

## Verification checklist (each change)

1. `cargo clippy && cargo test`
2. NPS comparison at depth 13 (see CLAUDE.md bench commands)
3. SPRT: `./bin/fastchess` tc=8+0.08, elo0=0 elo1=5, 15K rounds minimum
4. NNUE changes: validation loss on held-out data + SPRT

---

## Unresolved questions

1. Training data: use Stockfish or Lc0 data — no strong preference, either is fine for current strength level
2. ~~SMP thread count~~ — SMP scrapped for now (overcomplex for low return)
3. ~~SMP vs NNUE priority~~ — resolved: NNUE improvements next
4. No DFRC/Chess960 support planned
5. No specific Elo target or tournament — exploratory development
