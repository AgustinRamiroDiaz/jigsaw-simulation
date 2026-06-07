use serde::{Deserialize, Serialize};

use crate::Direction;

#[derive(Clone, Debug, Eq, PartialEq, Hash, Deserialize, Serialize)]
pub struct SideGuid(String);

impl SideGuid {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash, Deserialize, Serialize)]
pub struct Piece {
    sides: [SideGuid; 4],
}

impl Piece {
    pub fn new(sides: [SideGuid; 4]) -> Self {
        Self { sides }
    }

    pub fn side(&self, direction: Direction) -> &SideGuid {
        &self.sides[direction.index()]
    }

    pub fn rotate_clockwise(&self) -> Self {
        Self::new([
            self.sides[Direction::Left.index()].clone(),
            self.sides[Direction::Top.index()].clone(),
            self.sides[Direction::Right.index()].clone(),
            self.sides[Direction::Bottom.index()].clone(),
        ])
    }
}
