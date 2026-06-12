mod picking;
mod side_indexed;

pub use picking::{
    FirstAgainstRestPickingStrategy, PairPickingSolver, PickingStrategy, RandomPickingStrategy,
};
pub use side_indexed::SideIndexedSolver;

use crate::{
    Direction, Piece, Polyomino, PuzzleError, SideGuid, SolveStep, SolveTrace, TraceAction,
};

#[derive(Clone, Debug)]
pub(crate) enum SolverPhase {
    Initial,
    Running,
    Complete,
    Failed(PuzzleError),
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
    let mut solver = PairPickingSolver::new(pieces, seed)?;

    for step in solver.by_ref() {
        step?;
    }

    solver.solution().unwrap_or(Err(PuzzleError::CouldNotSolve))
}

pub fn solve_puzzle_with_trace(
    pieces: Vec<Piece>,
    seed: u64,
) -> Result<(Vec<Vec<Piece>>, SolveTrace), PuzzleError> {
    let mut solver = PairPickingSolver::new(pieces, seed)?;
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

pub(crate) fn join_first_available_pair(polyominos: &mut Vec<Polyomino>) -> Option<(usize, usize)> {
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

pub(crate) fn remove_pair<T>(
    items: &mut Vec<T>,
    first_index: usize,
    second_index: usize,
) -> (T, T) {
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

pub(crate) fn restore_pair<T>(
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

pub(crate) fn joined_index(first_index: usize, second_index: usize) -> usize {
    first_index - usize::from(second_index < first_index)
}

pub(crate) fn trace_step(
    attempt: usize,
    action: TraceAction,
    polyominos: &[Polyomino],
) -> SolveStep {
    SolveStep {
        attempt,
        action,
        polyominos: polyominos.iter().map(Polyomino::trace_snapshot).collect(),
    }
}
