# Oxid' Chess Engine — TODO / Roadmap (2625 → 2850+ Elo)

## Phase A — Low-hanging fruit (~+50 Elo)

- [x] **Continuation history** (+30-40 Elo) — shipped in v1.3.0
- [x] **Capture history** (+10-15 Elo) — shipped in v1.3.0
- [x] **Bad captures stage** (+5-10 Elo) — implemented in `src/search/move_picker.rs`

## Phase B (~+40 Elo cumulative)

- [ ] **Multi-bucket TT** (+30-50 Elo) — `src/evaluate/transposition.rs`
  - 3 entries per bucket (≤48 bytes, cache-line friendly)
  - Tighter packing: 16-bit key, u16 move, depth+gen+node_type packed
  - Replacement: always-replace shallowest/oldest; keep one depth-preferred slot
  - TT prefetch (`_mm_prefetch`) before `do_move`

- [ ] **PV tracking** (+5 Elo indirect) — `src/search.rs`
  - Add `pv_table: [[Move; MAX_PLY]; MAX_PLY]` and `pv_length`
  - Copy child PV on alpha improvement
  - Print full PV in UCI info output

## Phase C (~+15 Elo cumulative)

- [ ] **SIMD for NNUE** (+5-10 Elo via NPS) — `src/nnue/`
  - AVX2 intrinsics for accumulator ops and forward pass
  - SCReLU → `_mm256_min/max_epi16` + `_mm256_madd_epi16`

- [ ] **Singular extension tuning** (+5-10 Elo) — `src/search.rs`
  - Lower SE threshold: `depth >= 8` (from 10)
  - Double extensions when very singular (`s < se_beta - depth*2`)
  - Negative extensions on SE fail-high
  - Cap total extensions

- [ ] **Aspiration window tuning** (+2-5 Elo) — `src/search.rs`
  - Smaller initial window (±12-15cp)
  - Exponential widening (×2/×3) instead of additive (+100cp)
  - Don't reset best_move on fail-low

## Phase D (~+15 Elo cumulative)

- [ ] **Time management improvements** (+10-15 Elo) — `src/time.rs`
  - Node TM: stop early if best move has >90% root nodes, extend if <50%
  - Score stability: extend on score drops between iterations
  - Eval complexity: use root move score spread to estimate difficulty

## Phase E — Big features (~+80-130 Elo)

- [ ] **Self-play datagen pipeline** — new binary/command
  - `datagen` mode: self-play games → `.binpack` output
  - Params: num_games, depth (7-9), threads, random opening plies
  - Record (board, score, game_result) per position
  - Compatible with bullet sfbinpack loader in `training/src/main.rs`

- [ ] **Improved NNUE architecture** (+30-80 Elo) — `src/nnue/`, `training/`
  - Increase HIDDEN_SIZE to 384 or 512
  - Requires SIMD (Phase C) to keep NPS acceptable
  - Train on 200M+ self-play positions
  - Consider output buckets (material-based eval head selection)

- [ ] **End-game base tables / EGTB** (+10-15 Elo) — new module, `src/search.rs`
  - FFI to Fathom C library or pure-Rust Syzygy implementation
  - Root probe for DTZ (best winning move)
  - In-search WDL probe for score adjustment
  - Add `SyzygyPath` UCI option

- [ ] **Opening book support** — `src/uci.rs`, new module
  - Parse Polyglot `.bin` format
  - Add `OwnBook` and `BookFile` UCI options
  - Fall through to engine search when out of book

## Phase F — Parallelism (~+50-80 Elo at 4 threads)

- [ ] **Lazy SMP** — `src/search.rs`, `src/uci.rs`, `src/position.rs`
  - Convert `Rc<Bitboards>` and `Rc<Hasher>` to `Arc`
  - Shared TT via `Arc<TranspositionTable>` with lock-free entries
  - Each thread: own Search instance with separate history/killers
  - Main thread controls time via `AtomicBool` stop flag
  - Aggregate `nodes_searched` across threads

---

## Verification checklist (each change)

1. `cargo clippy && cargo test`
2. NPS comparison at depth 13 (see CLAUDE.md bench commands)
3. SPRT: `./bin/fastchess` tc=8+0.08, elo0=0 elo1=5, 15K rounds minimum
4. NNUE changes: validation loss on held-out data + SPRT

---

## Unresolved questions

1. Preferred net training dataset beyond self-play? (Lc0 data, Stockfish data, etc.)
2. Target thread count for SMP? (affects architecture — 4 vs 16+ threads)
3. Priority: SMP vs NNUE improvements? (SMP more effort but guaranteed; NNUE depends on data quality)
4. DFRC/Chess960 support? (affects datagen strategy)
5. Specific Elo target or tournament format? (affects time management tuning)
