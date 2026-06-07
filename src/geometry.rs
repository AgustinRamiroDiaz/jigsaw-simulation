use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd, Deserialize, Serialize)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    pub(crate) fn offset(self, delta: Point) -> Self {
        Self::new(self.x + delta.x, self.y + delta.y)
    }

    pub(crate) fn rotate_clockwise(self) -> Self {
        Self::new(-self.y, self.x)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub enum Direction {
    Top,
    Right,
    Bottom,
    Left,
}

impl Direction {
    pub(crate) const ALL: [Direction; 4] = [
        Direction::Top,
        Direction::Right,
        Direction::Bottom,
        Direction::Left,
    ];

    pub(crate) fn index(self) -> usize {
        match self {
            Direction::Top => 0,
            Direction::Right => 1,
            Direction::Bottom => 2,
            Direction::Left => 3,
        }
    }

    pub(crate) fn opposite(self) -> Self {
        match self {
            Direction::Top => Direction::Bottom,
            Direction::Right => Direction::Left,
            Direction::Bottom => Direction::Top,
            Direction::Left => Direction::Right,
        }
    }

    pub(crate) fn delta(self) -> Point {
        match self {
            Direction::Top => Point::new(0, -1),
            Direction::Right => Point::new(1, 0),
            Direction::Bottom => Point::new(0, 1),
            Direction::Left => Point::new(-1, 0),
        }
    }
}
