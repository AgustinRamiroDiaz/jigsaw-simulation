use std::{
    hint::black_box,
    time::{Duration, Instant},
};

use jigsaw_simulation::{
    FirstAgainstRestPickingStrategy, PairPickingSolver, PickingStrategy, Piece, PuzzleError,
    RandomPickingStrategy, SideIndexedSolver, generate_guid_grid, pieces_from_grid,
};

#[derive(Clone)]
struct PuzzleCase {
    name: &'static str,
    width: usize,
    height: usize,
    seed: u64,
    pieces: Vec<Piece>,
}

struct BenchResult {
    samples: Vec<Duration>,
    attempts: usize,
}

fn main() {
    let sample_count = env_usize("BENCH_SAMPLE_COUNT").unwrap_or(20);
    let warmup_count = env_usize("BENCH_WARMUP_COUNT").unwrap_or(3);
    let cases = [
        puzzle_case("small_4x4", 4, 4, 11),
        puzzle_case("medium_10x10", 10, 10, 42),
    ];

    println!("solver strategy benchmark: {sample_count} samples, {warmup_count} warmups per case");
    println!(
        "{:<14} {:<20} {:>10} {:>10} {:>10} {:>10}",
        "case", "strategy", "avg ms", "min ms", "max ms", "attempts"
    );

    for case in cases {
        run_case::<RandomPickingStrategyFactory>(&case, sample_count, warmup_count);
        run_case::<FirstAgainstRestPickingStrategyFactory>(&case, sample_count, warmup_count);
        run_case::<SideIndexedSolverFactory>(&case, sample_count, warmup_count);
    }
}

fn run_case<F: StrategyFactory>(case: &PuzzleCase, sample_count: usize, warmup_count: usize) {
    for _ in 0..warmup_count {
        black_box(F::solve(case.pieces.clone(), case.seed).expect("warmup should solve"));
    }

    let result = benchmark::<F>(case, sample_count);
    println!(
        "{:<14} {:<20} {:>10.3} {:>10.3} {:>10.3} {:>10}",
        format!("{}x{}", case.width, case.height),
        F::NAME,
        millis(avg(&result.samples)),
        millis(*result.samples.iter().min().expect("samples are present")),
        millis(*result.samples.iter().max().expect("samples are present")),
        result.attempts,
    );
    black_box(case.name);
}

fn benchmark<F: StrategyFactory>(case: &PuzzleCase, sample_count: usize) -> BenchResult {
    let mut samples = Vec::with_capacity(sample_count);
    let mut attempts = 0;

    for sample_index in 0..sample_count {
        let started = Instant::now();
        attempts = F::solve(case.pieces.clone(), case.seed + sample_index as u64)
            .expect("benchmark puzzle should solve");
        samples.push(started.elapsed());
    }

    BenchResult { samples, attempts }
}

fn solve(pieces: Vec<Piece>, strategy: Box<dyn PickingStrategy>) -> Result<usize, PuzzleError> {
    let mut solver = PairPickingSolver::with_picking_strategy(pieces, strategy)?;
    let mut attempts = 0;

    for step in solver.by_ref() {
        attempts = step?.attempt;
    }

    black_box(
        solver
            .solution()
            .unwrap_or(Err(PuzzleError::CouldNotSolve))?,
    );
    Ok(attempts)
}

fn puzzle_case(name: &'static str, width: usize, height: usize, seed: u64) -> PuzzleCase {
    let grid = generate_guid_grid(width, height);
    let mut pieces = pieces_from_grid(&grid);
    shuffle_and_rotate(&mut pieces, seed);
    PuzzleCase {
        name,
        width,
        height,
        seed,
        pieces,
    }
}

fn shuffle_and_rotate(pieces: &mut [Piece], seed: u64) {
    let mut rng = BenchRng::new(seed);

    pieces.iter_mut().for_each(|piece| {
        *piece = (0..rng.next_index(4)).fold(piece.clone(), |piece, _| piece.rotate_clockwise())
    });

    (1..pieces.len()).rev().for_each(|index| {
        let swap_index = rng.next_index(index + 1);
        pieces.swap(index, swap_index);
    });
}

fn env_usize(name: &str) -> Option<usize> {
    std::env::var(name).ok()?.parse().ok()
}

fn avg(samples: &[Duration]) -> Duration {
    samples.iter().sum::<Duration>() / samples.len() as u32
}

fn millis(duration: Duration) -> f64 {
    duration.as_secs_f64() * 1_000.0
}

trait StrategyFactory {
    const NAME: &'static str;
    fn solve(pieces: Vec<Piece>, seed: u64) -> Result<usize, PuzzleError>;
}

struct RandomPickingStrategyFactory;

impl StrategyFactory for RandomPickingStrategyFactory {
    const NAME: &'static str = "random";

    fn solve(pieces: Vec<Piece>, seed: u64) -> Result<usize, PuzzleError> {
        solve(pieces, Box::new(RandomPickingStrategy::new(seed)))
    }
}

struct FirstAgainstRestPickingStrategyFactory;

impl StrategyFactory for FirstAgainstRestPickingStrategyFactory {
    const NAME: &'static str = "first_against_rest";

    fn solve(pieces: Vec<Piece>, _seed: u64) -> Result<usize, PuzzleError> {
        solve(pieces, Box::new(FirstAgainstRestPickingStrategy::new()))
    }
}

struct SideIndexedSolverFactory;

impl StrategyFactory for SideIndexedSolverFactory {
    const NAME: &'static str = "side_indexed";

    fn solve(pieces: Vec<Piece>, _seed: u64) -> Result<usize, PuzzleError> {
        let mut solver = SideIndexedSolver::new(pieces)?;
        let mut attempts = 0;

        for step in solver.by_ref() {
            attempts = step?.attempt;
        }

        black_box(
            solver
                .solution()
                .unwrap_or(Err(PuzzleError::CouldNotSolve))?,
        );
        Ok(attempts)
    }
}

struct BenchRng {
    state: u64,
}

impl BenchRng {
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
