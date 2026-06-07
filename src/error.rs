#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PuzzleError {
    EmptyPuzzle,
    CouldNotSolve,
    InvalidShape,
    ShapeContainsHoles,
}
