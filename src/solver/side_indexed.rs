use std::collections::{HashMap, VecDeque};

use crate::{Direction, Piece, Point, Polyomino, PuzzleError, SideGuid, SolveStep, TraceAction};

use super::{SolverPhase, trace_step};

/// Solver that indexes every exposed side by its GUID and directly joins chunks
/// that share a side.
///
/// The algorithm is a work queue over stable component IDs:
///
/// 1. Store each starting piece as a one-cell component.
/// 2. Index every side of every component by `SideGuid`.
/// 3. Pop an active component from `scan_queue`.
/// 4. Look up each of its side GUIDs in `side_index`.
/// 5. If another active component has the same GUID, try the needed
///    rotation/translation and join the pair.
/// 6. Mark the old components inactive, append the joined component, index its
///    sides, and queue the new component for a later scan.
///
/// `side_index` is cleaned lazily: entries for consumed components stay in the
/// index, and `is_active` filters them out when they are encountered.
#[derive(Debug)]
pub struct SideIndexedSolver {
    /// Arena of every component ID the solver has ever created. Active
    /// components are `Some(polyomino)`; consumed components are `None`.
    /// Keeping old slots gives `side_index` and `scan_queue` stable IDs without
    /// reindexing after every join.
    components: Vec<Option<Polyomino>>,
    /// Side GUID lookup table. Each side points to component IDs that have, or
    /// once had, that side somewhere on their boundary. Stale IDs are expected
    /// and filtered through `is_active`.
    side_index: HashMap<SideGuid, Vec<IndexedSide>>,
    /// FIFO work queue of component IDs that should be scanned for a match.
    /// The logical front is the back of the deque: the solver pops with
    /// `pop_back` and pushes newly joined components with `push_front`.
    scan_queue: VecDeque<usize>,
    /// Number of currently active components. Solving completes when this
    /// reaches one.
    active_count: usize,
    /// Number assigned to emitted join trace actions after the initial state.
    attempts: usize,
    /// Iterator lifecycle: before the start trace, actively running, complete,
    /// or failed with an error to report via `solution`.
    phase: SolverPhase,
}

#[derive(Clone, Debug)]
struct IndexedSide {
    /// Component ID in `components` for a polyomino that contains this side.
    component_id: usize,
}

#[derive(Clone)]
struct PolyominoEdge<'a> {
    point: Point,
    piece: &'a Piece,
    direction: Direction,
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
            scan_queue: (0..components.len()).rev().collect(),
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

        while let Some(component_id) = self.scan_queue.pop_back() {
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
            self.scan_queue.push_front(joined_id);

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
