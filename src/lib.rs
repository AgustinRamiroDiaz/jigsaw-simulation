use std::collections::HashMap;

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct SideGuid(String);

impl SideGuid {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    fn offset(self, delta: Point) -> Self {
        Self::new(self.x + delta.x, self.y + delta.y)
    }

    fn rotate_clockwise(self) -> Self {
        Self::new(-self.y, self.x)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Direction {
    Top,
    Right,
    Bottom,
    Left,
}

impl Direction {
    const ALL: [Direction; 4] = [
        Direction::Top,
        Direction::Right,
        Direction::Bottom,
        Direction::Left,
    ];

    fn index(self) -> usize {
        match self {
            Direction::Top => 0,
            Direction::Right => 1,
            Direction::Bottom => 2,
            Direction::Left => 3,
        }
    }

    fn opposite(self) -> Self {
        match self {
            Direction::Top => Direction::Bottom,
            Direction::Right => Direction::Left,
            Direction::Bottom => Direction::Top,
            Direction::Left => Direction::Right,
        }
    }

    fn delta(self) -> Point {
        match self {
            Direction::Top => Point::new(0, -1),
            Direction::Right => Point::new(1, 0),
            Direction::Bottom => Point::new(0, 1),
            Direction::Left => Point::new(-1, 0),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
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

    pub fn piece_at(&self, point: Point) -> Option<&Piece> {
        self.cells.get(&point)
    }

    pub fn try_join(&self, other: &Polyomino) -> Option<Polyomino> {
        for rotated in other.rotations() {
            for (self_point, self_piece) in &self.cells {
                for (other_point, other_piece) in &rotated.cells {
                    for direction in Direction::ALL {
                        if self_piece.side(direction) != other_piece.side(direction.opposite()) {
                            continue;
                        }

                        let offset = Point::new(
                            self_point.x + direction.delta().x - other_point.x,
                            self_point.y + direction.delta().y - other_point.y,
                        );

                        if let Some(joined) = self.join_translated(&rotated, offset) {
                            return Some(joined);
                        }
                    }
                }
            }
        }

        None
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
        let mut grid = Vec::with_capacity(height);

        for y in 0..height {
            let mut row = Vec::with_capacity(width);
            for x in 0..width {
                let point = Point::new(x as i32, y as i32);
                let piece = normalized
                    .cells
                    .get(&point)
                    .ok_or(PuzzleError::ShapeContainsHoles)?;
                row.push(piece.clone());
            }
            grid.push(row);
        }

        Ok(grid)
    }

    fn rotations(&self) -> Vec<Polyomino> {
        let mut rotations = Vec::with_capacity(4);
        let mut current = self.clone();
        for _ in 0..4 {
            if !rotations.iter().any(|existing| existing == &current) {
                rotations.push(current.clone());
            }
            current = current.rotate_clockwise();
        }
        rotations
    }

    fn rotate_clockwise(&self) -> Self {
        let cells = self
            .cells
            .iter()
            .map(|(point, piece)| (point.rotate_clockwise(), piece.rotate_clockwise()))
            .collect();
        Self::from_cells(cells)
    }

    fn join_translated(&self, other: &Polyomino, offset: Point) -> Option<Polyomino> {
        let mut cells = self.cells.clone();

        for (point, piece) in &other.cells {
            let translated = point.offset(offset);
            if cells.contains_key(&translated) {
                return None;
            }
            cells.insert(translated, piece.clone());
        }

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

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PuzzleError {
    EmptyPuzzle,
    CouldNotSolve,
    InvalidShape,
    ShapeContainsHoles,
}

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
    if pieces.is_empty() {
        return Err(PuzzleError::EmptyPuzzle);
    }

    let mut rng = SimpleRng::new(seed);
    let mut polyominos: Vec<Polyomino> = pieces.into_iter().map(Polyomino::from_piece).collect();
    let mut failed_attempts = 0;
    let max_failed_attempts = polyominos.len().saturating_mul(polyominos.len()).max(16) * 128;

    while polyominos.len() > 1 {
        let first_index = rng.next_index(polyominos.len());
        let first = polyominos.swap_remove(first_index);
        let second_index = rng.next_index(polyominos.len());
        let second = polyominos.swap_remove(second_index);

        if let Some(joined) = first.try_join(&second) {
            polyominos.push(joined);
            failed_attempts = 0;
            continue;
        }

        polyominos.push(first);
        polyominos.push(second);
        failed_attempts += 1;

        if failed_attempts >= max_failed_attempts {
            if !join_first_available_pair(&mut polyominos) {
                return Err(PuzzleError::CouldNotSolve);
            }
            failed_attempts = 0;
        }
    }

    polyominos.pop().unwrap().to_grid()
}

pub fn assert_grid_has_matching_neighbors(grid: &[Vec<Piece>]) {
    for (y, row) in grid.iter().enumerate() {
        for (x, piece) in row.iter().enumerate() {
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
        }
    }
}

fn join_first_available_pair(polyominos: &mut Vec<Polyomino>) -> bool {
    for first_index in 0..polyominos.len() {
        for second_index in (first_index + 1)..polyominos.len() {
            if let Some(joined) = polyominos[first_index].try_join(&polyominos[second_index]) {
                polyominos.swap_remove(second_index);
                polyominos.swap_remove(first_index);
                polyominos.push(joined);
                return true;
            }
        }
    }

    false
}

fn all_touching_edges_match(cells: &HashMap<Point, Piece>) -> bool {
    for (point, piece) in cells {
        for direction in [Direction::Right, Direction::Bottom] {
            let neighbor_point = point.offset(direction.delta());
            if let Some(neighbor) = cells.get(&neighbor_point) {
                if piece.side(direction) != neighbor.side(direction.opposite()) {
                    return false;
                }
            }
        }
    }

    true
}

struct SimpleRng {
    state: u64,
}

impl SimpleRng {
    fn new(seed: u64) -> Self {
        Self { state: seed.max(1) }
    }

    fn next_index(&mut self, len: usize) -> usize {
        self.state ^= self.state << 13;
        self.state ^= self.state >> 7;
        self.state ^= self.state << 17;
        (self.state as usize) % len
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn side(id: &str) -> SideGuid {
        SideGuid::new(id)
    }

    #[test]
    fn converts_grid_of_side_guids_into_piece_set() {
        let grid = vec![
            vec![
                Piece::new([
                    side("a-top"),
                    side("a-right"),
                    side("a-bottom"),
                    side("a-left"),
                ]),
                Piece::new([
                    side("b-top"),
                    side("b-right"),
                    side("b-bottom"),
                    side("a-right"),
                ]),
            ],
            vec![
                Piece::new([
                    side("a-bottom"),
                    side("c-right"),
                    side("c-bottom"),
                    side("c-left"),
                ]),
                Piece::new([
                    side("b-bottom"),
                    side("d-right"),
                    side("d-bottom"),
                    side("c-right"),
                ]),
            ],
        ];

        let pieces = pieces_from_grid(&grid);

        assert_eq!(pieces.len(), 4);
        assert!(pieces.contains(&grid[0][0]));
        assert!(pieces.contains(&grid[0][1]));
        assert!(pieces.contains(&grid[1][0]));
        assert!(pieces.contains(&grid[1][1]));
    }

    #[test]
    fn joins_two_single_square_polyominos_when_matching_sides_touch() {
        let left = Piece::new([side("lt"), side("shared"), side("lb"), side("ll")]);
        let right = Piece::new([side("rt"), side("rr"), side("rb"), side("shared")]);

        let joined = Polyomino::from_piece(left.clone())
            .try_join(&Polyomino::from_piece(right.clone()))
            .expect("pieces should join on their shared side");

        assert_eq!(joined.len(), 2);
        assert_eq!(joined.piece_at(Point::new(0, 0)), Some(&left));
        assert_eq!(joined.piece_at(Point::new(1, 0)), Some(&right));
    }

    #[test]
    fn joining_supports_rotating_polyominos() {
        let base = Piece::new([
            side("base-top"),
            side("match"),
            side("base-bottom"),
            side("base-left"),
        ]);
        let rotated_neighbor = Piece::new([
            side("neighbor-left"),
            side("neighbor-top"),
            side("match"),
            side("neighbor-bottom"),
        ]);

        let joined = Polyomino::from_piece(base.clone())
            .try_join(&Polyomino::from_piece(rotated_neighbor.clone()))
            .expect("the second polyomino can rotate so its matching side faces left");

        assert_eq!(joined.len(), 2);
        assert_eq!(joined.piece_at(Point::new(0, 0)), Some(&base));
        assert_eq!(
            joined.piece_at(Point::new(1, 0)),
            Some(&rotated_neighbor.rotate_clockwise())
        );
    }

    #[test]
    fn joining_considers_holes_in_existing_polyomino_shapes() {
        let anchor = Piece::new([side("top-hole"), side("right-hole"), side("ab"), side("al")]);
        let top_left = Piece::new([side("tlt"), side("tlr"), side("top-hole"), side("tll")]);
        let top_right = Piece::new([side("trt"), side("trr"), side("trb"), side("tlr")]);
        let middle_right = Piece::new([side("trb"), side("mrr"), side("mrb"), side("right-hole")]);

        let with_hole = Polyomino::from_cells(vec![
            (Point::new(0, 0), top_left),
            (Point::new(1, 0), top_right),
            (Point::new(1, 1), middle_right),
        ]);

        let joined = with_hole
            .try_join(&Polyomino::from_piece(anchor.clone()))
            .expect("single piece should fit into the missing middle-left cell");

        assert_eq!(joined.len(), 4);
        assert_eq!(joined.piece_at(Point::new(0, 1)), Some(&anchor));
    }

    #[test]
    fn solves_generated_grid_back_into_a_complete_piece_grid() {
        let rows = 10;
        let cols = 10;
        let grid = generate_guid_grid(rows, cols);
        let mut pieces = pieces_from_grid(&grid);
        let mut rng = SimpleRng::new(42);

        for piece in &mut pieces {
            for _ in 0..rng.next_index(4) {
                *piece = piece.rotate_clockwise();
            }
        }

        for index in (1..pieces.len()).rev() {
            let swap_index = rng.next_index(index + 1);
            pieces.swap(index, swap_index);
        }

        let solved = solve_puzzle(pieces, 1).expect("generated puzzle should solve");

        assert_grid_has_matching_neighbors(&solved);
        assert_eq!(solved.len(), rows);
        assert_eq!(solved[0].len(), cols);
    }
}
