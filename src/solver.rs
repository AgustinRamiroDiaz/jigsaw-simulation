use std::collections::{HashMap, VecDeque};

use crate::{
    Direction, Piece, Point, Polyomino, PuzzleError, SideGuid, SolveStep, SolveTrace, TraceAction,
};

use crate::rng::SimpleRng;

pub trait PickingStrategy: std::fmt::Debug {
    fn pick(&mut self, polyomino_count: usize) -> Option<(usize, usize)>;
}

impl<T: PickingStrategy + ?Sized> PickingStrategy for Box<T> {
    fn pick(&mut self, polyomino_count: usize) -> Option<(usize, usize)> {
        (**self).pick(polyomino_count)
    }
}

#[derive(Debug)]
pub struct RandomPickingStrategy {
    rng: SimpleRng,
}

impl RandomPickingStrategy {
    pub fn new(seed: u64) -> Self {
        Self {
            rng: SimpleRng::new(seed),
        }
    }
}

impl PickingStrategy for RandomPickingStrategy {
    fn pick(&mut self, polyomino_count: usize) -> Option<(usize, usize)> {
        if polyomino_count < 2 {
            return None;
        }

        let first_index = self.rng.next_index(polyomino_count);
        let second_index = (0..(polyomino_count - 1))
            .map(|offset| (first_index + 1 + offset) % polyomino_count)
            .nth(self.rng.next_index(polyomino_count - 1))
            .expect("there is at least one second polyomino");

        Some((first_index, second_index))
    }
}

#[derive(Debug)]
pub struct FirstAgainstRestPickingStrategy {
    next_second_index: usize,
}

impl FirstAgainstRestPickingStrategy {
    pub fn new() -> Self {
        Self {
            next_second_index: 1,
        }
    }
}

impl Default for FirstAgainstRestPickingStrategy {
    fn default() -> Self {
        Self::new()
    }
}

impl PickingStrategy for FirstAgainstRestPickingStrategy {
    fn pick(&mut self, polyomino_count: usize) -> Option<(usize, usize)> {
        if polyomino_count < 2 {
            return None;
        }

        if self.next_second_index >= polyomino_count {
            self.next_second_index = 1;
        }

        let second_index = self.next_second_index;
        self.next_second_index += 1;

        Some((0, second_index))
    }
}

#[derive(Debug)]
pub struct PuzzleSolver {
    picking_strategy: Box<dyn PickingStrategy>,
    polyominos: Vec<Polyomino>,
    failed_attempts: usize,
    attempts: usize,
    max_failed_attempts: usize,
    phase: SolverPhase,
}

#[derive(Debug)]
pub struct SideIndexedSolver {
    components: Vec<Option<Polyomino>>,
    side_index: HashMap<SideGuid, Vec<IndexedSide>>,
    scan_queue: VecDeque<usize>,
    active_count: usize,
    attempts: usize,
    phase: SolverPhase,
}

#[derive(Clone, Debug)]
struct IndexedSide {
    component_id: usize,
}

#[derive(Clone, Debug)]
enum SolverPhase {
    Initial,
    Running,
    Complete,
    Failed(PuzzleError),
}

#[derive(Clone)]
struct PolyominoEdge<'a> {
    point: Point,
    piece: &'a Piece,
    direction: Direction,
}

impl PuzzleSolver {
    pub fn new(pieces: Vec<Piece>, seed: u64) -> Result<Self, PuzzleError> {
        Self::with_picking_strategy(pieces, RandomPickingStrategy::new(seed))
    }

    pub fn with_picking_strategy(
        pieces: Vec<Piece>,
        picking_strategy: impl PickingStrategy + 'static,
    ) -> Result<Self, PuzzleError> {
        if pieces.is_empty() {
            return Err(PuzzleError::EmptyPuzzle);
        }

        let polyominos: Vec<Polyomino> = pieces.into_iter().map(Polyomino::from_piece).collect();
        let max_failed_attempts = polyominos
            .len()
            .saturating_mul(polyominos.len())
            .max(16)
            .saturating_mul(128);

        Ok(Self {
            picking_strategy: Box::new(picking_strategy),
            polyominos,
            failed_attempts: 0,
            attempts: 0,
            max_failed_attempts,
            phase: SolverPhase::Initial,
        })
    }

    pub fn solution(&self) -> Option<Result<Vec<Vec<Piece>>, PuzzleError>> {
        match &self.phase {
            SolverPhase::Complete => Some(
                self.polyominos
                    .first()
                    .expect("solver has final polyomino")
                    .to_grid(),
            ),
            SolverPhase::Failed(error) => Some(Err(error.clone())),
            SolverPhase::Initial | SolverPhase::Running => None,
        }
    }

    fn advance(&mut self) -> Result<Option<SolveStep>, PuzzleError> {
        if self.polyominos.len() <= 1 {
            self.phase = SolverPhase::Complete;
            return Ok(None);
        }

        let Some((first_index, second_index)) = self.picking_strategy.pick(self.polyominos.len())
        else {
            self.phase = SolverPhase::Failed(PuzzleError::CouldNotSolve);
            return Err(PuzzleError::CouldNotSolve);
        };

        if first_index == second_index
            || first_index >= self.polyominos.len()
            || second_index >= self.polyominos.len()
        {
            self.phase = SolverPhase::Failed(PuzzleError::CouldNotSolve);
            return Err(PuzzleError::CouldNotSolve);
        }

        let (first, second) = remove_pair(&mut self.polyominos, first_index, second_index);
        self.attempts += 1;

        if let Some(joined) = first.try_join(&second) {
            self.polyominos
                .insert(joined_index(first_index, second_index), joined);
            self.failed_attempts = 0;
            return Ok(Some(trace_step(
                self.attempts,
                TraceAction::Joined {
                    first_index,
                    second_index,
                },
                &self.polyominos,
            )));
        }

        restore_pair(
            &mut self.polyominos,
            first_index,
            first,
            second_index,
            second,
        );
        self.failed_attempts += 1;
        let rejected = trace_step(
            self.attempts,
            TraceAction::Rejected {
                first_index,
                second_index,
            },
            &self.polyominos,
        );

        if self.failed_attempts < self.max_failed_attempts {
            return Ok(Some(rejected));
        }

        self.attempts += 1;
        let Some((first_index, second_index)) = join_first_available_pair(&mut self.polyominos)
        else {
            self.phase = SolverPhase::Failed(PuzzleError::CouldNotSolve);
            return Err(PuzzleError::CouldNotSolve);
        };

        self.failed_attempts = 0;
        Ok(Some(trace_step(
            self.attempts,
            TraceAction::FallbackJoined {
                first_index,
                second_index,
            },
            &self.polyominos,
        )))
    }
}

impl Iterator for PuzzleSolver {
    type Item = Result<SolveStep, PuzzleError>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.phase {
            SolverPhase::Initial => {
                self.phase = SolverPhase::Running;
                Some(Ok(trace_step(0, TraceAction::Started, &self.polyominos)))
            }
            SolverPhase::Running => match self.advance() {
                Ok(Some(step)) => Some(Ok(step)),
                Ok(None) => None,
                Err(error) => Some(Err(error)),
            },
            SolverPhase::Complete | SolverPhase::Failed(_) => None,
        }
    }
}

impl SideIndexedSolver {
    pub fn new(pieces: Vec<Piece>) -> Result<Self, PuzzleError> {
        if pieces.is_empty() {
            return Err(PuzzleError::EmptyPuzzle);
        }

        let components: Vec<_> = pieces
            .into_iter()
            .map(Polyomino::from_piece)
            .map(Some)
            .collect();
        let mut solver = Self {
            scan_queue: (0..components.len()).collect(),
            active_count: components.len(),
            components,
            side_index: HashMap::new(),
            attempts: 0,
            phase: SolverPhase::Initial,
        };

        (0..solver.components.len()).for_each(|component_id| {
            solver.index_component(component_id);
        });

        Ok(solver)
    }

    pub fn solution(&self) -> Option<Result<Vec<Vec<Piece>>, PuzzleError>> {
        match &self.phase {
            SolverPhase::Complete => self
                .components
                .iter()
                .find_map(Option::as_ref)
                .map(Polyomino::to_grid),
            SolverPhase::Failed(error) => Some(Err(error.clone())),
            SolverPhase::Initial | SolverPhase::Running => None,
        }
    }

    fn advance(&mut self) -> Result<Option<SolveStep>, PuzzleError> {
        if self.active_count <= 1 {
            self.phase = SolverPhase::Complete;
            return Ok(None);
        }

        while let Some(component_id) = self.scan_queue.pop_front() {
            if !self.is_active(component_id) {
                continue;
            }

            let Some(match_result) = self.find_match(component_id) else {
                continue;
            };

            self.attempts += 1;
            let first_index = self
                .active_index(component_id)
                .expect("matched component is active");
            let second_index = self
                .active_index(match_result.component_id)
                .expect("matched component is active");
            let joined_id = self.components.len();
            self.components[component_id] = None;
            self.components[match_result.component_id] = None;
            self.components.push(Some(match_result.joined));
            self.active_count -= 1;
            self.index_component(joined_id);
            self.scan_queue.push_back(joined_id);

            return Ok(Some(trace_step(
                self.attempts,
                TraceAction::Joined {
                    first_index,
                    second_index,
                },
                &self.active_polyominos(),
            )));
        }

        self.phase = SolverPhase::Failed(PuzzleError::CouldNotSolve);
        Err(PuzzleError::CouldNotSolve)
    }

    fn find_match(&self, component_id: usize) -> Option<IndexedMatch> {
        let component = self.component(component_id)?;

        polyomino_edges(component).into_iter().find_map(|edge| {
            self.side_index
                .get(edge.piece.side(edge.direction))?
                .iter()
                .find_map(|indexed_side| {
                    if indexed_side.component_id == component_id
                        || !self.is_active(indexed_side.component_id)
                    {
                        return None;
                    }

                    let other = self.component(indexed_side.component_id)?;
                    join_on_indexed_side(component, other, &edge).map(|joined| IndexedMatch {
                        component_id: indexed_side.component_id,
                        joined,
                    })
                })
        })
    }

    fn index_component(&mut self, component_id: usize) {
        let Some(component) = self.component(component_id) else {
            return;
        };

        let indexed_sides: Vec<_> = polyomino_edges(component)
            .into_iter()
            .map(|edge| {
                (
                    edge.piece.side(edge.direction).clone(),
                    IndexedSide { component_id },
                )
            })
            .collect();

        indexed_sides.into_iter().for_each(|(side, indexed_side)| {
            self.side_index.entry(side).or_default().push(indexed_side);
        });
    }

    fn component(&self, component_id: usize) -> Option<&Polyomino> {
        self.components.get(component_id)?.as_ref()
    }

    fn is_active(&self, component_id: usize) -> bool {
        self.components
            .get(component_id)
            .is_some_and(Option::is_some)
    }

    fn active_index(&self, component_id: usize) -> Option<usize> {
        self.components
            .iter()
            .enumerate()
            .filter(|(_, component)| component.is_some())
            .position(|(id, _)| id == component_id)
    }

    fn active_polyominos(&self) -> Vec<Polyomino> {
        self.components.iter().filter_map(Clone::clone).collect()
    }
}

impl Iterator for SideIndexedSolver {
    type Item = Result<SolveStep, PuzzleError>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.phase {
            SolverPhase::Initial => {
                self.phase = SolverPhase::Running;
                Some(Ok(trace_step(
                    0,
                    TraceAction::Started,
                    &self.active_polyominos(),
                )))
            }
            SolverPhase::Running => match self.advance() {
                Ok(Some(step)) => Some(Ok(step)),
                Ok(None) => None,
                Err(error) => Some(Err(error)),
            },
            SolverPhase::Complete | SolverPhase::Failed(_) => None,
        }
    }
}

struct IndexedMatch {
    component_id: usize,
    joined: Polyomino,
}

fn polyomino_edges(polyomino: &Polyomino) -> Vec<PolyominoEdge<'_>> {
    polyomino
        .cells()
        .flat_map(|(point, piece)| {
            Direction::ALL
                .into_iter()
                .map(move |direction| PolyominoEdge {
                    point: *point,
                    piece,
                    direction,
                })
        })
        .collect()
}

fn join_on_indexed_side(
    first: &Polyomino,
    second: &Polyomino,
    first_edge: &PolyominoEdge<'_>,
) -> Option<Polyomino> {
    let side = first_edge.piece.side(first_edge.direction);

    second.rotations().into_iter().find_map(|rotated| {
        polyomino_edges(&rotated)
            .into_iter()
            .filter(|rotated_edge| {
                rotated_edge.direction == first_edge.direction.opposite()
                    && rotated_edge.piece.side(rotated_edge.direction) == side
            })
            .find_map(|rotated_edge| {
                let offset = Point::new(
                    first_edge.point.x + first_edge.direction.delta().x - rotated_edge.point.x,
                    first_edge.point.y + first_edge.direction.delta().y - rotated_edge.point.y,
                );
                first.join_translated(&rotated, offset)
            })
    })
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
    let mut solver = PuzzleSolver::new(pieces, seed)?;

    for step in solver.by_ref() {
        step?;
    }

    solver.solution().unwrap_or(Err(PuzzleError::CouldNotSolve))
}

pub fn solve_puzzle_with_trace(
    pieces: Vec<Piece>,
    seed: u64,
) -> Result<(Vec<Vec<Piece>>, SolveTrace), PuzzleError> {
    let mut solver = PuzzleSolver::new(pieces, seed)?;
    let mut trace = SolveTrace { steps: Vec::new() };

    for step in solver.by_ref() {
        trace.steps.push(step?);
    }

    solver
        .solution()
        .unwrap_or(Err(PuzzleError::CouldNotSolve))
        .map(|grid| (grid, trace))
}

pub fn assert_grid_has_matching_neighbors(grid: &[Vec<Piece>]) {
    grid.iter().enumerate().for_each(|(y, row)| {
        row.iter().enumerate().for_each(|(x, piece)| {
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
        });
    });
}

fn join_first_available_pair(polyominos: &mut Vec<Polyomino>) -> Option<(usize, usize)> {
    (0..polyominos.len())
        .find_map(|first_index| {
            ((first_index + 1)..polyominos.len()).find_map(|second_index| {
                polyominos[first_index]
                    .try_join(&polyominos[second_index])
                    .map(|joined| (first_index, second_index, joined))
            })
        })
        .map(|(first_index, second_index, joined)| {
            remove_pair(polyominos, first_index, second_index);
            polyominos.insert(joined_index(first_index, second_index), joined);
            (first_index, second_index)
        })
}

fn remove_pair<T>(items: &mut Vec<T>, first_index: usize, second_index: usize) -> (T, T) {
    if first_index > second_index {
        let first = items.remove(first_index);
        let second = items.remove(second_index);
        (first, second)
    } else {
        let second = items.remove(second_index);
        let first = items.remove(first_index);
        (first, second)
    }
}

fn restore_pair<T>(
    items: &mut Vec<T>,
    first_index: usize,
    first: T,
    second_index: usize,
    second: T,
) {
    if first_index > second_index {
        items.insert(second_index, second);
        items.insert(first_index, first);
    } else {
        items.insert(first_index, first);
        items.insert(second_index, second);
    }
}

fn joined_index(first_index: usize, second_index: usize) -> usize {
    first_index - usize::from(second_index < first_index)
}

fn trace_step(attempt: usize, action: TraceAction, polyominos: &[Polyomino]) -> SolveStep {
    SolveStep {
        attempt,
        action,
        polyominos: polyominos.iter().map(Polyomino::trace_snapshot).collect(),
    }
}
