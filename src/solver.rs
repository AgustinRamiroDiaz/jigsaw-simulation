use crate::{
    Direction, Piece, Polyomino, PuzzleError, SideGuid, SolveStep, SolveTrace, TraceAction,
};

use crate::rng::SimpleRng;

#[derive(Debug)]
pub struct PuzzleSolver {
    rng: SimpleRng,
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
        if pieces.is_empty() {
            return Err(PuzzleError::EmptyPuzzle);
        }

        let polyominos: Vec<Polyomino> = pieces.into_iter().map(Polyomino::from_piece).collect();
        let max_failed_attempts = polyominos.len().saturating_mul(polyominos.len()).max(16) * 128;

        Ok(Self {
            rng: SimpleRng::new(seed),
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

        let first_index = self.rng.next_index(self.polyominos.len());
        let first = self.polyominos.swap_remove(first_index);
        let second_index = self.rng.next_index(self.polyominos.len());
        let second = self.polyominos.swap_remove(second_index);
        self.attempts += 1;

        if let Some(joined) = first.try_join(&second) {
            self.polyominos.push(joined);
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

        self.polyominos.push(first);
        self.polyominos.push(second);
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
            polyominos.swap_remove(second_index);
            polyominos.swap_remove(first_index);
            polyominos.push(joined);
            (first_index, second_index)
        })
}

fn trace_step(attempt: usize, action: TraceAction, polyominos: &[Polyomino]) -> SolveStep {
    SolveStep {
        attempt,
        action,
        polyominos: polyominos.iter().map(Polyomino::trace_snapshot).collect(),
    }
}
