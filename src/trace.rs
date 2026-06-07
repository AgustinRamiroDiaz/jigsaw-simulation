use serde::{Deserialize, Serialize};

use crate::{Piece, Point};

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct TraceCell {
    pub point: Point,
    pub piece: Piece,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct TracePolyomino {
    pub cells: Vec<TraceCell>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub enum TraceAction {
    Started,
    Joined {
        first_index: usize,
        second_index: usize,
    },
    Rejected {
        first_index: usize,
        second_index: usize,
    },
    FallbackJoined {
        first_index: usize,
        second_index: usize,
    },
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct SolveStep {
    pub attempt: usize,
    pub action: TraceAction,
    pub polyominos: Vec<TracePolyomino>,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub struct SolveTrace {
    pub steps: Vec<SolveStep>,
}

impl SolveTrace {
    pub fn last_step(&self) -> Option<&SolveStep> {
        self.steps.last()
    }
}
