use std::fmt::{self, Display};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SolverStrategy {
    Random,
    FirstAgainstRest,
    SideIndexed,
}

impl SolverStrategy {
    pub(crate) const ALL: [Self; 3] = [Self::Random, Self::FirstAgainstRest, Self::SideIndexed];

    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    pub(crate) fn from_query_value(value: &str) -> Option<Self> {
        match value {
            "random" | "random-pair" => Some(Self::Random),
            "first" | "first-against-rest" | "first_against_rest" => Some(Self::FirstAgainstRest),
            "side-indexed" | "side_indexed" | "indexed" => Some(Self::SideIndexed),
            _ => None,
        }
    }
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
