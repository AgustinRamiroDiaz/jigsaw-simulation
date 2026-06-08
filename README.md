# Jigsaw Simulation

A Rust simulation of a jigsaw puzzle solver.

The puzzle is modeled as a rectangular grid of pieces. Each piece has four
sides, and each side has a GUID-like identifier. Two pieces can connect when
the identifiers on the touching sides match. Pieces are represented as four
side IDs in this order:

```text
[top, right, bottom, left]
```

The solver converts pieces into one-cell polyominos, repeatedly tries to join
random pairs, handles rotations, and stops when one complete polyomino remains.

## Workspace

This repository is a Cargo workspace with two packages:

```text
jigsaw-simulation   Core puzzle model, solver, tests, and trace API
trace-viewer        Iced GUI for stepping through solver trace snapshots
```

## Algorithm

The core solver follows this process:

```text
start with the set of all pieces
convert them to polyominos of just one square
loop until only one polyomino is left:
- pick 2 random polyominos
- check all possible rotations and placements
- if they can be joined, join them and put the result back into the set
- otherwise, put both polyominos back into the set
convert the final polyomino into a grid of pieces
```

Polyominos can contain holes, and the join logic considers those holes when
testing candidate placements.

## Trace API

The core crate exposes two solver entry points:

```rust
solve_puzzle(pieces, seed)
solve_puzzle_with_trace(pieces, seed)
```

`solve_puzzle_with_trace` returns both the solved grid and a `SolveTrace`.
The trace contains ordered `SolveStep` snapshots, including:

- the attempt number
- the action taken: started, joined, rejected, or fallback joined
- the current set of polyominos after that step

This keeps the solver testable while giving the viewer everything it needs to
visualize the algorithm.

## Trace Viewer

Run the Iced viewer with:

```bash
cargo run -p trace-viewer
```

The viewer generates a sample puzzle, randomly rotates and shuffles the pieces,
runs the traced solver, and lets you step through the algorithm with:

- First
- Previous
- Next
- Last

Each square is drawn as one puzzle piece. Each polyomino is laid out separately
so you can watch the set shrink as successful joins occur.

## Web Build

The viewer can also be built for the web with Trunk:

```bash
trunk build trace-viewer/index.html --release --public-url "/jigsaw-simulation/"
```

For local development:

```bash
trunk serve trace-viewer/index.html
```

The project uses Iced 0.14 with the `webgl` feature for the WASM build. The
older `iced_web` crate exists, but it targets the historical Iced 0.4 stack, so
it is not used by this workspace.

GitHub Pages publishing is handled by `.github/workflows/pages.yml`.

## Development

Run the core tests:

```bash
cargo test -p jigsaw-simulation
```

Check the viewer:

```bash
cargo check -p trace-viewer
```

Format the workspace:

```bash
cargo fmt
```

Run everything you usually need:

```bash
cargo test -p jigsaw-simulation
cargo check -p trace-viewer
```

## Benchmarks and Profiling

Compare the random strategy with the first-against-rest strategy:

```bash
cargo bench --bench solver_strategies
```

The benchmark uses 20 measured samples by default. You can tune that without
editing code:

```bash
BENCH_SAMPLE_COUNT=50 BENCH_WARMUP_COUNT=5 cargo bench --bench solver_strategies
```

Run an optimized profiling workload for one or both strategies:

```bash
cargo run --profile profiling --bin solver_profile -- --strategy both --width 10 --height 10 --iterations 10
```

The profiler prints elapsed time and solver counters, and can be wrapped by
system profilers such as `perf`:

```bash
perf record --call-graph=dwarf cargo run --profile profiling --bin solver_profile -- --strategy random --width 10 --height 10 --iterations 100
```
