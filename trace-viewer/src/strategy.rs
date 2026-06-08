use std::fmt::{self, Display};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SolverStrategy {
    Random,
    FirstAgainstRest,
    SideIndexed,
}

impl SolverStrategy {
    pub(crate) const ALL: [Self; 3] = [Self::Random, Self::FirstAgainstRest, Self::SideIndexed];
}

impl Display for SolverStrategy {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SolverStrategy::Random => formatter.write_str("Random pair"),
            SolverStrategy::FirstAgainstRest => formatter.write_str("First against rest"),
            SolverStrategy::SideIndexed => formatter.write_str("Side indexed"),
        }
    }
}
