# UCI Protocol

Oxid' implements the [Universal Chess Interface](https://en.wikipedia.org/wiki/Universal_Chess_Interface) protocol. It does not include a GUI — use a UCI-compatible interface such as [Cute Chess](https://cutechess.com/) or [Arena](http://www.playwitharena.de/).

## Supported Commands

### `uci`
Identifies the engine. Prints engine name, author, available options, and `uciok`.

### `isready`
Responds with `readyok` when the engine is ready.

### `ucinewgame`
Resets the position to the starting position and clears the transposition table.

### `position startpos [moves ...]`
Sets the board to the starting position, then applies the given move sequence.

### `position fen <fenstring> [moves ...]`
Sets the board to the given FEN, then applies the given move sequence.

### `go [options]`
Starts searching. Supported options:

| Option | Description |
|--------|-------------|
| `depth <n>` | Search to a fixed depth |
| `movetime <ms>` | Search for a fixed time |
| `wtime <ms>` | White's remaining time |
| `btime <ms>` | Black's remaining time |
| `winc <ms>` | White's increment per move |
| `binc <ms>` | Black's increment per move |
| `movestogo <n>` | Moves until next time control |
| `nodes <n>` | Search up to N nodes |
| `infinite` | Search until `stop` |
| `ponder` | Search in pondering mode |
| `perft <n>` | Run perft to depth N |

### `stop`
Stops the current search (handled by time manager cutoff).

### `setoption name Hash value <n>`
Sets the transposition table size in MB (1-512). Default: 16 MB.

### `setoption name EvalFile value <path>`
Loads a NNUE network from a file path, replacing the embedded default. Useful for SPRT testing candidate nets without recompiling. If the file cannot be loaded, the current net is kept.

### `setoption name SyzygyPath value <path>`
Sets the path to Syzygy endgame tablebases. Accepts a colon-separated list of directories (e.g. `./syzygy/345:./syzygy/6`). When loaded, the engine probes DTZ tables at the root for optimal move selection and WDL tables during search for exact endgame scores. Only probes positions with piece count within the loaded tables' range and no castling rights. Set to `<empty>` to disable.

### `bench [hash_size] [threads] [depth]`
Runs the benchmark suite (46 positions). Default depth: 13. Reports total nodes, time, and nodes/second.

### `datagen [depth] [num_games] [output_path]`
Generates self-play training data for NNUE training. Plays games against itself with random openings (8 random plies), fixed-depth search, and writes positions to a plain text file. Default: depth 8, 1000 games, `data/selfplay.txt`. Output format: `FEN;MOVE;SCORE;PLY;WDL` (one position per line). Use `tools/plain2binpack` to convert to Stockfish binpack format for training.

### `quit`
Exits the engine.

## Info Output

During search, the engine reports progress:

```
info depth 12 seldepth 18 multipv 1 score cp 35 nodes 482910 nps 1205000 hashfull 142 tbhits 0 time 401 pv e2e4 e7e5 ...
```

## Best Move Output

```
bestmove e2e4 ponder e7e5
```
