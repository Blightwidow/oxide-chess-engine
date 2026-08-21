// Headless smoke test for the wasm build. Run after scripts/build_wasm.sh:
//
//   node wasm/harness.mjs
//
// Asserts that init/legal_moves/best_move work without panicking and that the
// returned move is actually legal in the position that was passed in.

import { readdirSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const repoRoot = join(dirname(fileURLToPath(import.meta.url)), "..");
const packageDir = join(repoRoot, "pkg");

const START_POSITION = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
const MOVETIME_MS = 1000;

const failures = [];
function check(description, condition, detail = "") {
  if (condition) {
    console.log(`  ok   ${description}`);
  } else {
    console.log(`  FAIL ${description}${detail ? ` — ${detail}` : ""}`);
    failures.push(description);
  }
}

// --target web glue fetches the wasm by URL by default, which does not work for
// file paths under Node. Hand it the bytes instead.
const { default: initWasm, init } = await import(join(packageDir, "oxid.js"));
await initWasm({ module_or_path: readFileSync(join(packageDir, "oxid_bg.wasm")) });

const netDir = join(packageDir, "nets");
const netName = readdirSync(netDir).find((entry) => entry.endsWith(".nnue"));
if (!netName) {
  console.error(`no .nnue found in ${netDir} — run scripts/build_wasm.sh first`);
  process.exit(1);
}
const netBytes = readFileSync(join(netDir, netName));
console.log(`net ${netName} (${netBytes.length} bytes)`);

const engine = init(netBytes);
check("init() returned a handle", engine !== undefined && engine !== null);

const legalMoves = engine.legal_moves(START_POSITION);
check("legal_moves() returns 20 moves from the start position", legalMoves.length === 20, `got ${legalMoves.length}`);
check("legal_moves() includes e2e4", legalMoves.includes("e2e4"));
check("legal_moves() includes g1f3", legalMoves.includes("g1f3"));

const malformed = engine.legal_moves("not-a-fen");
check("legal_moves() rejects a malformed FEN instead of panicking", Array.isArray(malformed) && malformed.length === 0);

const searchStart = performance.now();
const bestMove = engine.best_move(START_POSITION, MOVETIME_MS);
const searchMs = Math.round(performance.now() - searchStart);
console.log(`best_move(startpos, ${MOVETIME_MS}) = ${bestMove} (took ${searchMs} ms)`);
check("best_move() returned a move", typeof bestMove === "string" && bestMove.length >= 4);
// If web-time were not wired up the search would fall out immediately (or panic),
// so a wall-clock time in the right ballpark is the real clock assertion.
check(
  "best_move() actually used its time budget (web-time clock is live)",
  searchMs > MOVETIME_MS * 0.4 && searchMs < MOVETIME_MS * 3,
  `took ${searchMs} ms for a ${MOVETIME_MS} ms budget`,
);
check("best_move() returned a legal move", legalMoves.includes(bestMove), `${bestMove} not in legal move list`);

// Checkmate: no move to make, and it must not panic.
const mated = engine.best_move("7k/5KQ1/8/8/8/8/8/8 b - - 0 1", 200);
check("best_move() returns an empty string in a terminal position", mated === "", `got "${mated}"`);

// A second search on the same handle must still work (state is reset per search).
const secondMove = engine.best_move("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1", 200);
check("best_move() is reusable across calls", secondMove.length >= 4, `got "${secondMove}"`);

if (failures.length > 0) {
  console.error(`\n${failures.length} check(s) failed`);
  process.exit(1);
}
console.log("\nall checks passed");
