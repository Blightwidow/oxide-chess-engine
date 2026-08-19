# WebAssembly Build

The engine core compiles to `wasm32-unknown-unknown` so a browser can play against
it. The build is feature-gated: everything that needs a terminal, a filesystem or
linked C code is excluded.

## Cargo layout

`src/lib.rs` owns the module tree; `src/main.rs` is a thin binary on top of it.
The crate builds both an `rlib` and a `cdylib`.

| Feature | Default | Contents |
|---------|---------|----------|
| `host` | on | `uci`, `benchmark`, `eret`, `datagen` — stdin loop, bench suite, EPD tester, self-play datagen |
| `tablebase` | on | `tablebase` module. Off for wasm because `pyrrhic-rs` links C sources |
| `wasm` | off | `src/wasm.rs`, the wasm-bindgen shim |

The `oxid` binary declares `required-features = ["host"]`, so a wasm build skips
it instead of failing to compile.

## Building

```bash
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version <version from Cargo.lock> --locked

./scripts/build_wasm.sh    # -> pkg/{oxid.js, oxid_bg.wasm, nets/*.nnue}
node wasm/harness.mjs      # headless smoke test
```

The underlying command is:

```bash
cargo build --release --target wasm32-unknown-unknown --no-default-features --features wasm
```

`.cargo/config.toml` sets `-C target-cpu=native` under `[build]`, which is invalid
for wasm. The `[target.wasm32-unknown-unknown]` table *replaces* that (tables do
not merge), substituting `-C target-feature=+simd128`. Do not set a `RUSTFLAGS`
env var for a wasm build — env overrides config wholesale and would drop the flag.

## JS API

The net is **not** embedded in the `.wasm` blob — that keeps it at ~156 KB
instead of ~3.3 MB. The page fetches the net and passes the bytes in.

```js
import initWasm, { init } from "./oxid.js";

await initWasm();
const netBytes = new Uint8Array(
  await (await fetch("./nets/nn-8808c22a8203.nnue")).arrayBuffer(),
);

const engine = init(netBytes);                  // -> Engine handle
engine.legal_moves(fen);                        // -> string[] of UCI moves
engine.best_move(fen, 1000);                    // -> UCI move, searched for 1000 ms
```

Create one `Engine` and reuse it: it owns the magic bitboard tables, the
transposition table and the NNUE weights, so per-call setup would dominate.

`legal_moves` returns `[]` and `best_move` returns `""` for a malformed FEN, and
`best_move` returns `""` in a terminal position (checkmate or stalemate). An
incompatible net falls back to zero weights, which still plays legal moves.

## Clocks

`std::time::Instant::now()` and `SystemTime::now()` both panic on
`wasm32-unknown-unknown` — there is no platform clock. Every clock read goes
through `src/clock.rs`, which re-exports std on native targets and
[`web-time`](https://crates.io/crates/web-time) on wasm, where the types are
backed by `performance.now()` and `Date.now()`. `web-time` is a wasm32-only
dependency, so native builds are unchanged.

## Not implemented

- **SIMD NNUE kernels.** `-C target-feature=+simd128` is enabled, but
  `src/nnue/simd.rs` still takes the scalar fallback path on wasm. Hand-written
  `core::arch::wasm32` intrinsics are the obvious next speedup.
- **Threads.** Single-threaded only; `wasm32-unknown-unknown` has no thread
  support without `SharedArrayBuffer` plus COOP/COEP headers.
- **Syzygy tablebases and the Polyglot book file.** The book module compiles but
  its `std::fs` read always fails in a browser, so `Search::book` stays `None`.

## Release artifact

The `wasm` job in `.github/workflows/release.yml` runs the build script, executes
the Node harness as a gate, and publishes `oxide-wasm.tar.gz` (wasm + JS glue +
`nets/*.nnue`) as a release asset.
