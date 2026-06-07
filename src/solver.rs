use crate::{
    Direction, Piece, Polyomino, PuzzleError, SideGuid, SolveStep, SolveTrace, TraceAction,
};

use crate::rng::SimpleRng;

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
    solve_puzzle_with_trace(pieces, seed).map(|(grid, _trace)| grid)
}

pub fn solve_puzzle_with_trace(
    pieces: Vec<Piece>,
    seed: u64,
) -> Result<(Vec<Vec<Piece>>, SolveTrace), PuzzleError> {
    if pieces.is_empty() {
        return Err(PuzzleError::EmptyPuzzle);
    }

    let mut rng = SimpleRng::new(seed);
    let mut polyominos: Vec<Polyomino> = pieces.into_iter().map(Polyomino::from_piece).collect();
    let mut failed_attempts = 0;
    let mut attempts = 0;
    let mut trace = SolveTrace {
        steps: vec![trace_step(0, TraceAction::Started, &polyominos)],
    };
    let max_failed_attempts = polyominos.len().saturating_mul(polyominos.len()).max(16) * 128;

    while polyominos.len() > 1 {
        let first_index = rng.next_index(polyominos.len());
        let first = polyominos.swap_remove(first_index);
        let second_index = rng.next_index(polyominos.len());
        let second = polyominos.swap_remove(second_index);
        attempts += 1;

        if let Some(joined) = first.try_join(&second) {
            polyominos.push(joined);
            failed_attempts = 0;
            trace.steps.push(trace_step(
                attempts,
                TraceAction::Joined {
                    first_index,
                    second_index,
                },
                &polyominos,
            ));
            continue;
        }

        polyominos.push(first);
        polyominos.push(second);
        failed_attempts += 1;
        trace.steps.push(trace_step(
            attempts,
            TraceAction::Rejected {
                first_index,
                second_index,
            },
            &polyominos,
        ));

        if failed_attempts >= max_failed_attempts {
            attempts += 1;
            if let Some((first_index, second_index)) = join_first_available_pair(&mut polyominos) {
                trace.steps.push(trace_step(
                    attempts,
                    TraceAction::FallbackJoined {
                        first_index,
                        second_index,
                    },
                    &polyominos,
                ));
            } else {
                return Err(PuzzleError::CouldNotSolve);
            }
            failed_attempts = 0;
        }
    }

    polyominos
        .pop()
        .unwrap()
        .to_grid()
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
