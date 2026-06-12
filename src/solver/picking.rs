use crate::rng::SimpleRng;
use crate::{Piece, Polyomino, PuzzleError, SolveStep, TraceAction};

use super::{
    SolverPhase, join_first_available_pair, joined_index, remove_pair, restore_pair, trace_step,
};

/// Chooses which two active polyominos the pair-picking solver should try next.
pub trait PickingStrategy: std::fmt::Debug {
    fn pick(&mut self, polyomino_count: usize) -> Option<(usize, usize)>;
}

impl<T: PickingStrategy + ?Sized> PickingStrategy for Box<T> {
    fn pick(&mut self, polyomino_count: usize) -> Option<(usize, usize)> {
        (**self).pick(polyomino_count)
    }
}

/// Picks a random first polyomino, then a random different second polyomino.
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

/// Repeatedly tries to join the first active polyomino against the rest.
#[derive(Debug)]
pub struct FirstAgainstRestPickingStrategy {
    /// Next candidate index to try against index 0.
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

/// Solver that repeatedly picks two active chunks and asks `Polyomino::try_join`
/// whether any rotation/translation can make their matching sides touch.
///
/// The solver is simple and useful for tracing because every failed pair can be
/// emitted as a rejected step. It is also intentionally less direct than
/// `SideIndexedSolver`: it does not know which sides might match until it tries
/// the selected pair.
#[derive(Debug)]
pub struct PairPickingSolver {
    /// Policy used to choose the next pair of active polyominos to test.
    picking_strategy: Box<dyn PickingStrategy>,
    /// Current active puzzle chunks. On a successful join, the two inputs are
    /// removed and the combined polyomino is inserted near their original
    /// position so trace indices remain easy to follow.
    polyominos: Vec<Polyomino>,
    /// Consecutive rejected pair attempts since the last successful join.
    failed_attempts: usize,
    /// Number assigned to emitted trace actions after the initial state.
    attempts: usize,
    /// Guardrail for strategies that may keep choosing non-matching pairs.
    /// Once this many consecutive rejections happen, the solver performs one
    /// exhaustive fallback scan and joins the first available pair.
    max_failed_attempts: usize,
    /// Iterator lifecycle: before the start trace, actively running, complete,
    /// or failed with an error to report via `solution`.
    phase: SolverPhase,
}

impl PairPickingSolver {
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
        let max_failed_attempts = polyominos
            .len()
            .saturating_mul(polyominos.len())
            .max(16)
            .saturating_mul(128);

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

impl Iterator for PairPickingSolver {
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
