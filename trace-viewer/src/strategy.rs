use std::fmt::{self, Display};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SolverStrategy {
    Random,
    FirstAgainstRest,
}

impl SolverStrategy {
    pub(crate) const ALL: [Self; 2] = [Self::Random, Self::FirstAgainstRest];
}

impl Display for SolverStrategy {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SolverStrategy::Random => formatter.write_str("Random pair"),
            SolverStrategy::FirstAgainstRest => formatter.write_str("First against rest"),
        }
    }
}
