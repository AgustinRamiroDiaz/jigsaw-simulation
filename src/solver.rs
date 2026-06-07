use crate::{
    Direction, Piece, Polyomino, PuzzleError, SideGuid, SolveStep, SolveTrace, TraceAction,
};

use crate::rng::SimpleRng;

pub trait PickingStrategy: std::fmt::Debug {
    fn pick(&mut self, polyomino_count: usize) -> Option<(usize, usize)>;
}

#[derive(Debug)]
pub struct RandomPickingStrategy {
    rng: SimpleRng,
}

impl RandomPickingStrategy {
    pub fn new(seed: u64) -> Self {
        Self {
            rng: SimpleRng::new(seed),
        }
    }
}

impl PickingStrategy for RandomPickingStrategy {
    fn pick(&mut self, polyomino_count: usize) -> Option<(usize, usize)> {
        if polyomino_count < 2 {
            return None;
        }

        let first_index = self.rng.next_index(polyomino_count);
        let second_index = (0..(polyomino_count - 1))
            .map(|offset| (first_index + 1 + offset) % polyomino_count)
            .nth(self.rng.next_index(polyomino_count - 1))
            .expect("there is at least one second polyomino");

        Some((first_index, second_index))
    }
}

#[derive(Debug)]
pub struct FirstAgainstRestPickingStrategy {
    next_second_index: usize,
}

impl FirstAgainstRestPickingStrategy {
    pub fn new() -> Self {
        Self {
            next_second_index: 1,
        }
    }
}

impl Default for FirstAgainstRestPickingStrategy {
    fn default() -> Self {
        Self::new()
    }
}

impl PickingStrategy for FirstAgainstRestPickingStrategy {
    fn pick(&mut self, polyomino_count: usize) -> Option<(usize, usize)> {
        if polyomino_count < 2 {
            return None;
        }

        if self.next_second_index >= polyomino_count {
            self.next_second_index = 1;
        }

        let second_index = self.next_second_index;
        self.next_second_index += 1;

        Some((0, second_index))
    }
}

#[derive(Debug)]
pub struct PuzzleSolver {
    picking_strategy: Box<dyn PickingStrategy>,
    polyominos: Vec<Polyomino>,
    failed_attempts: usize,
    attempts: usize,
    max_failed_attempts: usize,
    phase: SolverPhase,
}

#[derive(Clone, Debug)]
enum SolverPhase {
    Initial,
    Running,
    Complete,
    Failed(PuzzleError),
}

impl PuzzleSolver {
    pub fn new(pieces: Vec<Piece>, seed: u64) -> Result<Self, PuzzleError> {
        Self::with_picking_strategy(pieces, RandomPickingStrategy::new(seed))
    }

    pub fn with_picking_strategy(
        pieces: Vec<Piece>,
        picking_strategy: impl PickingStrategy + 'static,
    ) -> Result<Self, PuzzleError> {
        if pieces.is_empty() {
            return Err(PuzzleError::EmptyPuzzle);
        }

        let polyominos: Vec<Polyomino> = pieces.into_iter().map(Polyomino::from_piece).collect();
        let max_failed_attempts = polyominos.len().saturating_mul(polyominos.len()).max(16) * 128;

        Ok(Self {
            picking_strategy: Box::new(picking_strategy),
            polyominos,
            failed_attempts: 0,
            attempts: 0,
            max_failed_attempts,
            phase: SolverPhase::Initial,
        })
    }

    pub fn solution(&self) -> Option<Result<Vec<Vec<Piece>>, PuzzleError>> {
        match &self.phase {
            SolverPhase::Complete => Some(
                self.polyominos
                    .first()
                    .expect("solver has final polyomino")
                    .to_grid(),
            ),
            SolverPhase::Failed(error) => Some(Err(error.clone())),
            SolverPhase::Initial | SolverPhase::Running => None,
        }
    }

    fn advance(&mut self) -> Result<Option<SolveStep>, PuzzleError> {
        if self.polyominos.len() <= 1 {
            self.phase = SolverPhase::Complete;
            return Ok(None);
        }

        let Some((first_index, second_index)) = self.picking_strategy.pick(self.polyominos.len())
        else {
            self.phase = SolverPhase::Failed(PuzzleError::CouldNotSolve);
            return Err(PuzzleError::CouldNotSolve);
        };

        if first_index == second_index
            || first_index >= self.polyominos.len()
            || second_index >= self.polyominos.len()
        {
            self.phase = SolverPhase::Failed(PuzzleError::CouldNotSolve);
            return Err(PuzzleError::CouldNotSolve);
        }

        let (first, second) = remove_pair(&mut self.polyominos, first_index, second_index);
        self.attempts += 1;

        if let Some(joined) = first.try_join(&second) {
            self.polyominos
                .insert(joined_index(first_index, second_index), joined);
            self.failed_attempts = 0;
            return Ok(Some(trace_step(
                self.attempts,
                TraceAction::Joined {
                    first_index,
                    second_index,
                },
                &self.polyominos,
            )));
        }

        restore_pair(
            &mut self.polyominos,
            first_index,
            first,
            second_index,
            second,
        );
        self.failed_attempts += 1;
        let rejected = trace_step(
            self.attempts,
            TraceAction::Rejected {
                first_index,
                second_index,
            },
            &self.polyominos,
        );

        if self.failed_attempts < self.max_failed_attempts {
            return Ok(Some(rejected));
        }

        self.attempts += 1;
        let Some((first_index, second_index)) = join_first_available_pair(&mut self.polyominos)
        else {
            self.phase = SolverPhase::Failed(PuzzleError::CouldNotSolve);
            return Err(PuzzleError::CouldNotSolve);
        };

        self.failed_attempts = 0;
        Ok(Some(trace_step(
            self.attempts,
            TraceAction::FallbackJoined {
                first_index,
                second_index,
            },
            &self.polyominos,
        )))
    }
}

impl Iterator for PuzzleSolver {
    type Item = Result<SolveStep, PuzzleError>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.phase {
            SolverPhase::Initial => {
                self.phase = SolverPhase::Running;
                Some(Ok(trace_step(0, TraceAction::Started, &self.polyominos)))
            }
            SolverPhase::Running => match self.advance() {
                Ok(Some(step)) => Some(Ok(step)),
                Ok(None) => None,
                Err(error) => Some(Err(error)),
            },
            SolverPhase::Complete | SolverPhase::Failed(_) => None,
        }
    }
}

pub fn generate_guid_grid(width: usize, height: usize) -> Vec<Vec<Piece>> {
    (0..height)
        .map(|y| {
            (0..width)
                .map(|x| {
                    let top = if y == 0 {
                        format!("border-top-{x}")
                    } else {
                        format!("vertical-{}-{x}", y - 1)
                    };
                    let right = if x + 1 == width {
                        format!("border-right-{y}")
                    } else {
                        format!("horizontal-{y}-{x}")
                    };
                    let bottom = if y + 1 == height {
                        format!("border-bottom-{x}")
                    } else {
                        format!("vertical-{y}-{x}")
                    };
                    let left = if x == 0 {
                        format!("border-left-{y}")
                    } else {
                        format!("horizontal-{}-{}", y, x - 1)
                    };

                    Piece::new([
                        SideGuid::new(top),
                        SideGuid::new(right),
                        SideGuid::new(bottom),
                        SideGuid::new(left),
                    ])
                })
                .collect()
        })
        .collect()
}

pub fn pieces_from_grid(grid: &[Vec<Piece>]) -> Vec<Piece> {
    grid.iter().flatten().cloned().collect()
}

pub fn solve_puzzle(pieces: Vec<Piece>, seed: u64) -> Result<Vec<Vec<Piece>>, PuzzleError> {
    let mut solver = PuzzleSolver::new(pieces, seed)?;

    for step in solver.by_ref() {
        step?;
    }

    solver.solution().unwrap_or(Err(PuzzleError::CouldNotSolve))
}

pub fn solve_puzzle_with_trace(
    pieces: Vec<Piece>,
    seed: u64,
) -> Result<(Vec<Vec<Piece>>, SolveTrace), PuzzleError> {
    let mut solver = PuzzleSolver::new(pieces, seed)?;
    let mut trace = SolveTrace { steps: Vec::new() };

    for step in solver.by_ref() {
        trace.steps.push(step?);
    }

    solver
        .solution()
        .unwrap_or(Err(PuzzleError::CouldNotSolve))
        .map(|grid| (grid, trace))
}

pub fn assert_grid_has_matching_neighbors(grid: &[Vec<Piece>]) {
    grid.iter().enumerate().for_each(|(y, row)| {
        row.iter().enumerate().for_each(|(x, piece)| {
            if x + 1 < row.len() {
                assert_eq!(
                    piece.side(Direction::Right),
                    grid[y][x + 1].side(Direction::Left)
                );
            }
            if y + 1 < grid.len() {
                assert_eq!(
                    piece.side(Direction::Bottom),
                    grid[y + 1][x].side(Direction::Top)
                );
            }
        });
    });
}

fn join_first_available_pair(polyominos: &mut Vec<Polyomino>) -> Option<(usize, usize)> {
    (0..polyominos.len())
        .find_map(|first_index| {
            ((first_index + 1)..polyominos.len()).find_map(|second_index| {
                polyominos[first_index]
                    .try_join(&polyominos[second_index])
                    .map(|joined| (first_index, second_index, joined))
            })
        })
        .map(|(first_index, second_index, joined)| {
            remove_pair(polyominos, first_index, second_index);
            polyominos.insert(joined_index(first_index, second_index), joined);
            (first_index, second_index)
        })
}

fn remove_pair<T>(items: &mut Vec<T>, first_index: usize, second_index: usize) -> (T, T) {
    if first_index > second_index {
        let first = items.remove(first_index);
        let second = items.remove(second_index);
        (first, second)
    } else {
        let second = items.remove(second_index);
        let first = items.remove(first_index);
        (first, second)
    }
}

fn restore_pair<T>(
    items: &mut Vec<T>,
    first_index: usize,
    first: T,
    second_index: usize,
    second: T,
) {
    if first_index > second_index {
        items.insert(second_index, second);
        items.insert(first_index, first);
    } else {
        items.insert(first_index, first);
        items.insert(second_index, second);
    }
}

fn joined_index(first_index: usize, second_index: usize) -> usize {
    first_index - usize::from(second_index < first_index)
}

fn trace_step(attempt: usize, action: TraceAction, polyominos: &[Polyomino]) -> SolveStep {
    SolveStep {
        attempt,
        action,
        polyominos: polyominos.iter().map(Polyomino::trace_snapshot).collect(),
    }
}
