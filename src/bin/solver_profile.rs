use std::{
    env,
    process::ExitCode,
    time::{Duration, Instant},
};

use jigsaw_simulation::{
    FirstAgainstRestPickingStrategy, PairPickingSolver, PickingStrategy, Piece, PuzzleError,
    RandomPickingStrategy, SideIndexedSolver, TraceAction, assert_grid_has_matching_neighbors,
    generate_guid_grid, pieces_from_grid,
};

#[derive(Clone, Copy)]
enum StrategyKind {
    Random,
    FirstAgainstRest,
    SideIndexed,
}

impl StrategyKind {
    fn parse(value: &str) -> Option<Self> {
        match value {
            "random" => Some(Self::Random),
            "first-against-rest" | "first_against_rest" | "first" => Some(Self::FirstAgainstRest),
            "side-indexed" | "side_indexed" | "indexed" => Some(Self::SideIndexed),
            _ => None,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Random => "random",
            Self::FirstAgainstRest => "first-against-rest",
            Self::SideIndexed => "side-indexed",
        }
    }

    fn run(self, pieces: Vec<Piece>, seed: u64) -> Result<RunResult, PuzzleError> {
        match self {
            Self::Random => run_picking_solver(pieces, Box::new(RandomPickingStrategy::new(seed))),
            Self::FirstAgainstRest => {
                run_picking_solver(pieces, Box::new(FirstAgainstRestPickingStrategy::new()))
            }
            Self::SideIndexed => run_side_indexed_solver(pieces),
        }
    }
}

struct Config {
    width: usize,
    height: usize,
    seed: u64,
    iterations: usize,
    strategies: Vec<StrategyKind>,
}

#[derive(Default)]
struct SolverCounts {
    attempts: usize,
    joined: usize,
    rejected: usize,
    fallback_joined: usize,
}

struct RunResult {
    elapsed: Duration,
    counts: SolverCounts,
}

fn main() -> ExitCode {
    let config = match Config::parse() {
        Ok(config) => config,
        Err(message) => {
            eprintln!("{message}");
            print_usage();
            return ExitCode::from(2);
        }
    };

    let mut pieces = pieces_from_grid(&generate_guid_grid(config.width, config.height));
    shuffle_and_rotate(&mut pieces, config.seed);

    for strategy in config.strategies {
        let mut total = Duration::ZERO;
        let mut last_counts = SolverCounts::default();

        for iteration in 0..config.iterations {
            let result = match strategy.run(pieces.clone(), config.seed + iteration as u64) {
                Ok(result) => result,
                Err(error) => {
                    eprintln!("{} failed: {error:?}", strategy.name());
                    return ExitCode::from(1);
                }
            };
            total += result.elapsed;
            last_counts = result.counts;
        }

        println!(
            "strategy={} size={}x{} iterations={} total_ms={:.3} avg_ms={:.3} attempts={} joined={} rejected={} fallback_joined={}",
            strategy.name(),
            config.width,
            config.height,
            config.iterations,
            total.as_secs_f64() * 1_000.0,
            total.as_secs_f64() * 1_000.0 / config.iterations as f64,
            last_counts.attempts,
            last_counts.joined,
            last_counts.rejected,
            last_counts.fallback_joined,
        );
    }

    ExitCode::SUCCESS
}

impl Config {
    fn parse() -> Result<Self, String> {
        let mut args = env::args().skip(1);
        let mut width = 10;
        let mut height = 10;
        let mut seed = 42;
        let mut iterations = 1;
        let mut strategies = all_strategies();

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--width" => width = parse_next(&mut args, "--width")?,
                "--height" => height = parse_next(&mut args, "--height")?,
                "--seed" => seed = parse_next(&mut args, "--seed")?,
                "--iterations" => iterations = parse_next(&mut args, "--iterations")?,
                "--strategy" => {
                    let value = args
                        .next()
                        .ok_or_else(|| "--strategy requires a value".to_string())?;
                    strategies = if value == "all" || value == "both" {
                        all_strategies()
                    } else {
                        vec![
                            StrategyKind::parse(&value)
                                .ok_or_else(|| format!("unknown strategy '{value}'"))?,
                        ]
                    };
                }
                "--help" | "-h" => return Err(String::new()),
                other => return Err(format!("unknown argument '{other}'")),
            }
        }

        if width == 0 || height == 0 {
            return Err("width and height must be greater than 0".to_string());
        }
        if iterations == 0 {
            return Err("iterations must be greater than 0".to_string());
        }

        Ok(Self {
            width,
            height,
            seed,
            iterations,
            strategies,
        })
    }
}

fn parse_next<T: std::str::FromStr>(
    args: &mut impl Iterator<Item = String>,
    name: &str,
) -> Result<T, String> {
    args.next()
        .ok_or_else(|| format!("{name} requires a value"))?
        .parse()
        .map_err(|_| format!("{name} has an invalid value"))
}

fn all_strategies() -> Vec<StrategyKind> {
    vec![
        StrategyKind::Random,
        StrategyKind::FirstAgainstRest,
        StrategyKind::SideIndexed,
    ]
}

fn run_picking_solver(
    pieces: Vec<Piece>,
    strategy: Box<dyn PickingStrategy>,
) -> Result<RunResult, PuzzleError> {
    let mut solver = PairPickingSolver::with_picking_strategy(pieces, strategy)?;
    let started = Instant::now();
    let mut counts = SolverCounts::default();

    for step in solver.by_ref() {
        let step = step?;
        counts.attempts = step.attempt;
        match step.action {
            TraceAction::Started => {}
            TraceAction::Joined { .. } => counts.joined += 1,
            TraceAction::Rejected { .. } => counts.rejected += 1,
            TraceAction::FallbackJoined { .. } => counts.fallback_joined += 1,
        }
    }

    let solved = solver
        .solution()
        .unwrap_or(Err(PuzzleError::CouldNotSolve))?;
    assert_grid_has_matching_neighbors(&solved);

    Ok(RunResult {
        elapsed: started.elapsed(),
        counts,
    })
}

fn run_side_indexed_solver(pieces: Vec<Piece>) -> Result<RunResult, PuzzleError> {
    let mut solver = SideIndexedSolver::new(pieces)?;
    let started = Instant::now();
    let mut counts = SolverCounts::default();

    for step in solver.by_ref() {
        let step = step?;
        counts.attempts = step.attempt;
        match step.action {
            TraceAction::Started => {}
            TraceAction::Joined { .. } => counts.joined += 1,
            TraceAction::Rejected { .. } => counts.rejected += 1,
            TraceAction::FallbackJoined { .. } => counts.fallback_joined += 1,
        }
    }

    let solved = solver
        .solution()
        .unwrap_or(Err(PuzzleError::CouldNotSolve))?;
    assert_grid_has_matching_neighbors(&solved);

    Ok(RunResult {
        elapsed: started.elapsed(),
        counts,
    })
}

fn shuffle_and_rotate(pieces: &mut [Piece], seed: u64) {
    let mut rng = ProfileRng::new(seed);

    pieces.iter_mut().for_each(|piece| {
        *piece = (0..rng.next_index(4)).fold(piece.clone(), |piece, _| piece.rotate_clockwise())
    });

    (1..pieces.len()).rev().for_each(|index| {
        let swap_index = rng.next_index(index + 1);
        pieces.swap(index, swap_index);
    });
}

fn print_usage() {
    eprintln!(
        "usage: cargo run --profile profiling --bin solver_profile -- [--strategy random|first-against-rest|side-indexed|all] [--width N] [--height N] [--seed N] [--iterations N]"
    );
}

struct ProfileRng {
    state: u64,
}

impl ProfileRng {
    fn new(seed: u64) -> Self {
        Self { state: seed.max(1) }
    }

    fn next_index(&mut self, len: usize) -> usize {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 7;
        self.state ^= self.state << 17;
        (self.state as usize) % len
    }
}
