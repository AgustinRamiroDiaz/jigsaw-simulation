mod error;
mod geometry;
mod piece;
mod polyomino;
mod rng;
mod solver;
mod trace;

pub use error::PuzzleError;
pub use geometry::{Direction, Point};
pub use piece::{Piece, SideGuid};
pub use polyomino::Polyomino;
pub use solver::{
    assert_grid_has_matching_neighbors, generate_guid_grid, pieces_from_grid, solve_puzzle,
    solve_puzzle_with_trace,
};
pub use trace::{SolveStep, SolveTrace, TraceAction, TraceCell, TracePolyomino};

#[cfg(test)]
mod tests;
