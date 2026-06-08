use std::collections::HashMap;

use crate::{Direction, Piece, Point, PuzzleError, TraceCell, TracePolyomino};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Polyomino {
    cells: HashMap<Point, Piece>,
}

impl Polyomino {
    pub fn from_piece(piece: Piece) -> Self {
        Self::from_cells(vec![(Point::new(0, 0), piece)])
    }

    pub fn from_cells(cells: Vec<(Point, Piece)>) -> Self {
        let mut polyomino = Self {
            cells: cells.into_iter().collect(),
        };
        polyomino.normalize();
        polyomino
    }

    pub fn len(&self) -> usize {
        self.cells.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }

    pub fn piece_at(&self, point: Point) -> Option<&Piece> {
        self.cells.get(&point)
    }

    pub(crate) fn cells(&self) -> impl Iterator<Item = (&Point, &Piece)> {
        self.cells.iter()
    }

    pub fn trace_snapshot(&self) -> TracePolyomino {
        let mut cells: Vec<_> = self
            .normalized_clone()
            .cells
            .into_iter()
            .map(|(point, piece)| TraceCell { point, piece })
            .collect();
        cells.sort_by_key(|cell| (cell.point.y, cell.point.x));
        TracePolyomino { cells }
    }

    pub fn try_join(&self, other: &Polyomino) -> Option<Polyomino> {
        other.rotations().into_iter().find_map(|rotated| {
            self.cells
                .iter()
                .flat_map(|(self_point, self_piece)| {
                    rotated
                        .cells
                        .iter()
                        .flat_map(move |(other_point, other_piece)| {
                            Direction::ALL.into_iter().map(move |direction| {
                                (self_point, self_piece, other_point, other_piece, direction)
                            })
                        })
                })
                .filter(|(_, self_piece, _, other_piece, direction)| {
                    self_piece.side(*direction) == other_piece.side(direction.opposite())
                })
                .map(|(self_point, _, other_point, _, direction)| {
                    Point::new(
                        self_point.x + direction.delta().x - other_point.x,
                        self_point.y + direction.delta().y - other_point.y,
                    )
                })
                .find_map(|offset| self.join_translated(&rotated, offset))
        })
    }

    pub fn to_grid(&self) -> Result<Vec<Vec<Piece>>, PuzzleError> {
        let normalized = self.normalized_clone();
        let max_x = normalized
            .cells
            .keys()
            .map(|point| point.x)
            .max()
            .unwrap_or(0);
        let max_y = normalized
            .cells
            .keys()
            .map(|point| point.y)
            .max()
            .unwrap_or(0);
        let width = usize::try_from(max_x + 1).map_err(|_| PuzzleError::InvalidShape)?;
        let height = usize::try_from(max_y + 1).map_err(|_| PuzzleError::InvalidShape)?;

        (0..height)
            .map(|y| {
                (0..width)
                    .map(|x| {
                        normalized
                            .cells
                            .get(&Point::new(x as i32, y as i32))
                            .cloned()
                            .ok_or(PuzzleError::ShapeContainsHoles)
                    })
                    .collect()
            })
            .collect()
    }

    pub(crate) fn rotations(&self) -> Vec<Polyomino> {
        (0..4)
            .scan(self.clone(), |current, _| {
                let rotation = current.clone();
                *current = current.rotate_clockwise();
                Some(rotation)
            })
            .fold(Vec::with_capacity(4), |mut rotations, rotation| {
                if !rotations.iter().any(|existing| existing == &rotation) {
                    rotations.push(rotation);
                }
                rotations
            })
    }

    fn rotate_clockwise(&self) -> Self {
        let cells = self
            .cells
            .iter()
            .map(|(point, piece)| (point.rotate_clockwise(), piece.rotate_clockwise()))
            .collect();
        Self::from_cells(cells)
    }

    pub(crate) fn join_translated(&self, other: &Polyomino, offset: Point) -> Option<Polyomino> {
        let cells =
            other
                .cells
                .iter()
                .try_fold(self.cells.clone(), |mut cells, (point, piece)| {
                    let translated = point.offset(offset);
                    (!cells.contains_key(&translated)).then(|| {
                        cells.insert(translated, piece.clone());
                        cells
                    })
                })?;

        if !all_touching_edges_match(&cells) {
            return None;
        }

        let mut joined = Polyomino { cells };
        joined.normalize();
        Some(joined)
    }

    fn normalized_clone(&self) -> Self {
        let mut clone = self.clone();
        clone.normalize();
        clone
    }

    fn normalize(&mut self) {
        if self.cells.is_empty() {
            return;
        }

        let min_x = self.cells.keys().map(|point| point.x).min().unwrap();
        let min_y = self.cells.keys().map(|point| point.y).min().unwrap();

        if min_x == 0 && min_y == 0 {
            return;
        }

        self.cells = self
            .cells
            .drain()
            .map(|(point, piece)| (Point::new(point.x - min_x, point.y - min_y), piece))
            .collect();
    }
}

fn all_touching_edges_match(cells: &HashMap<Point, Piece>) -> bool {
    cells.iter().all(|(point, piece)| {
        [Direction::Right, Direction::Bottom]
            .into_iter()
            .all(|direction| {
                cells
                    .get(&point.offset(direction.delta()))
                    .is_none_or(|neighbor| {
                        piece.side(direction) == neighbor.side(direction.opposite())
                    })
            })
    })
}
