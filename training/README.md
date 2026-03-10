# Training

## Generate Data

Use the Stockfish `tools` branch to generate training binpacks.

### Build Stockfish tools branch

```bash
git clone https://github.com/official-stockfish/Stockfish.git
cd Stockfish/src
git checkout tools
make -j profile-build ARCH=apple-silicon COMP=clang # use x86-64-bmi2 on Intel
```

### Generate binpacks

```
./stockfish
setoption name Threads value 6
setoption name Hash value 4096
setoption name Use NNUE value true
isready
generate_training_data depth 8 count 100000000 save_every 1000000 eval_limit 32000
```

- `depth` — search depth for scoring positions (8 is a good quality/speed tradeoff)
- `count` — number of positions to generate
- `save_every` — checkpoint interval (saves a new binpack file every N positions)
- `eval_limit` — discard positions with eval above this (filters out won/lost positions)

Copy the resulting `.binpack` files into `training/data/`.

### Download binpack

Alternatively, you could simply download pre-existing dataset like [the ones used by Stockfish](https://robotmoon.com/nnue-training-data/).

### Renaming binpacks

When merging binpacks from multiple generation runs, filenames may collide. Use the rename script to assign random UUID names before adding new files:

```bash
./training/rename_binpacks.sh
```

## Training

Add binpacks in data then run the following command

```bash
# I am runing cpu feature as I am using an apple-silicon Macbook for now
cargo run --release --features cpu --no-default-features --bin train
```

Then once you are happy, convert a checkpoint using

```bash
cargo run --release --bin convert <quantised.bin> ./nets/default.nnue
```

## SPRT Testing

After training a new net, run an SPRT test to verify it doesn't regress (or measures an Elo gain) against the current default net:

```bash
cargo build -r

./bin/fastchess \
  -engine cmd=./target/release/chessbot name=new_net "option.EvalFile=nets/new.nnue" \
  -engine cmd=./target/release/chessbot name=base_net "option.EvalFile=nets/default.nnue" \
  -openings file=./data/openings.pgn format=pgn order=random \
  -each tc=8+0.08 -rounds 15000 -repeat -concurrency 6 -recover \
  -sprt elo0=0 elo1=5 alpha=0.05 beta=0.05
```

Both engines use the same binary — only the `EvalFile` UCI option differs.