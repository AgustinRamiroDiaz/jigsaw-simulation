use crate::rng::SimpleRng;
use crate::*;

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

    pieces.iter_mut().for_each(|piece| {
        *piece = (0..rng.next_index(4)).fold(piece.clone(), |piece, _| piece.rotate_clockwise())
    });

    (1..pieces.len()).rev().for_each(|index| {
        let swap_index = rng.next_index(index + 1);
        pieces.swap(index, swap_index);
    });

    let solved = solve_puzzle(pieces, 1).expect("generated puzzle should solve");

    assert_grid_has_matching_neighbors(&solved);
    assert_eq!(solved.len(), rows);
    assert_eq!(solved[0].len(), cols);
}

#[test]
fn puzzle_solver_yields_started_step_first() {
    let grid = generate_guid_grid(2, 2);
    let pieces = pieces_from_grid(&grid);
    let mut solver = PuzzleSolver::new(pieces, 7).expect("solver should initialize");

    let first_step = solver
        .next()
        .expect("solver should yield initial step")
        .expect("initial step should succeed");

    assert_eq!(first_step.attempt, 0);
    assert!(matches!(first_step.action, TraceAction::Started));
    assert_eq!(first_step.polyominos.len(), 4);
    assert!(solver.solution().is_none());
}

#[test]
fn puzzle_solver_collected_steps_match_trace_solver() {
    let grid = generate_guid_grid(3, 2);
    let pieces = pieces_from_grid(&grid);
    let mut solver = PuzzleSolver::new(pieces.clone(), 7).expect("solver should initialize");

    let collected_steps = solver
        .by_ref()
        .collect::<Result<Vec<_>, _>>()
        .expect("iterator should solve");
    let (_, eager_trace) = solve_puzzle_with_trace(pieces, 7).expect("eager trace should solve");

    assert_eq!(collected_steps, eager_trace.steps);
}

#[test]
fn puzzle_solver_solution_is_available_after_completion() {
    let grid = generate_guid_grid(4, 3);
    let pieces = pieces_from_grid(&grid);
    let mut solver = PuzzleSolver::new(pieces, 11).expect("solver should initialize");

    solver
        .by_ref()
        .try_for_each(|step| step.map(|_| ()))
        .expect("iterator should solve");

    let solved = solver
        .solution()
        .expect("solution should be available after iterator completes")
        .expect("solution should be valid");

    assert_grid_has_matching_neighbors(&solved);
    assert_eq!(solved.len(), 3);
    assert_eq!(solved[0].len(), 4);
}

#[test]
fn first_against_rest_strategy_tracks_next_candidate() {
    let mut strategy = FirstAgainstRestPickingStrategy::new();

    assert_eq!(strategy.pick(4), Some((0, 1)));
    assert_eq!(strategy.pick(4), Some((0, 2)));
    assert_eq!(strategy.pick(4), Some((0, 3)));
    assert_eq!(strategy.pick(4), Some((0, 1)));
    assert_eq!(strategy.pick(2), Some((0, 1)));
    assert_eq!(strategy.pick(1), None);
}

#[test]
fn puzzle_solver_can_use_first_against_rest_strategy() {
    let grid = generate_guid_grid(4, 3);
    let pieces = pieces_from_grid(&grid);
    let mut solver =
        PuzzleSolver::with_picking_strategy(pieces, FirstAgainstRestPickingStrategy::new())
            .expect("solver should initialize");

    solver
        .by_ref()
        .try_for_each(|step| step.map(|_| ()))
        .expect("iterator should solve");

    let solved = solver
        .solution()
        .expect("solution should be available after iterator completes")
        .expect("solution should be valid");

    assert_grid_has_matching_neighbors(&solved);
    assert_eq!(solved.len(), 3);
    assert_eq!(solved[0].len(), 4);
}

#[test]
fn side_indexed_solver_yields_started_step_first() {
    let grid = generate_guid_grid(2, 2);
    let pieces = pieces_from_grid(&grid);
    let mut solver = SideIndexedSolver::new(pieces).expect("solver should initialize");

    let first_step = solver
        .next()
        .expect("solver should yield initial step")
        .expect("initial step should succeed");

    assert_eq!(first_step.attempt, 0);
    assert!(matches!(first_step.action, TraceAction::Started));
    assert_eq!(first_step.polyominos.len(), 4);
    assert!(solver.solution().is_none());
}

#[test]
fn side_indexed_solver_solves_generated_grid() {
    let rows = 10;
    let cols = 10;
    let grid = generate_guid_grid(cols, rows);
    let mut pieces = pieces_from_grid(&grid);
    let mut rng = SimpleRng::new(42);

    pieces.iter_mut().for_each(|piece| {
        *piece = (0..rng.next_index(4)).fold(piece.clone(), |piece, _| piece.rotate_clockwise())
    });

    (1..pieces.len()).rev().for_each(|index| {
        let swap_index = rng.next_index(index + 1);
        pieces.swap(index, swap_index);
    });

    let mut solver = SideIndexedSolver::new(pieces).expect("solver should initialize");
    solver
        .by_ref()
        .try_for_each(|step| step.map(|_| ()))
        .expect("iterator should solve");

    let solved = solver
        .solution()
        .expect("solution should be available after iterator completes")
        .expect("solution should be valid");

    assert_grid_has_matching_neighbors(&solved);
    assert_eq!(solved.len(), rows);
    assert_eq!(solved[0].len(), cols);
}

#[test]
fn first_against_rest_rejections_keep_the_first_polyomino_stable() {
    let pieces = vec![
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
            side("b-left"),
        ]),
        Piece::new([
            side("c-top"),
            side("c-right"),
            side("c-bottom"),
            side("c-left"),
        ]),
    ];
    let mut solver =
        PuzzleSolver::with_picking_strategy(pieces, FirstAgainstRestPickingStrategy::new())
            .expect("solver should initialize");

    let initial = solver
        .next()
        .expect("solver should yield initial step")
        .expect("initial step should succeed");
    let first_rejection = solver
        .next()
        .expect("solver should yield first attempt")
        .expect("first rejection should succeed");
    let second_rejection = solver
        .next()
        .expect("solver should yield second attempt")
        .expect("second rejection should succeed");

    assert!(matches!(
        first_rejection.action,
        TraceAction::Rejected {
            first_index: 0,
            second_index: 1
        }
    ));
    assert!(matches!(
        second_rejection.action,
        TraceAction::Rejected {
            first_index: 0,
            second_index: 2
        }
    ));
    assert_eq!(first_rejection.polyominos, initial.polyominos);
    assert_eq!(second_rejection.polyominos, initial.polyominos);
}

#[test]
fn first_against_rest_keeps_all_non_first_polyominos_as_single_pieces_on_large_grid() {
    let grid = generate_guid_grid(10, 10);
    let pieces = pieces_from_grid(&grid);
    let mut solver =
        PuzzleSolver::with_picking_strategy(pieces, FirstAgainstRestPickingStrategy::new())
            .expect("solver should initialize");

    solver
        .by_ref()
        .try_for_each(|step| {
            let step = step?;
            assert!(
                step.polyominos
                    .iter()
                    .skip(1)
                    .all(|polyomino| polyomino.cells.len() == 1),
                "step {} should only grow the first polyomino",
                step.attempt
            );
            Ok::<_, PuzzleError>(())
        })
        .expect("iterator should solve");

    let solved = solver
        .solution()
        .expect("solution should be available after iterator completes")
        .expect("solution should be valid");

    assert_grid_has_matching_neighbors(&solved);
    assert_eq!(solved.len(), 10);
    assert_eq!(solved[0].len(), 10);
}

#[test]
fn solving_with_trace_records_algorithm_snapshots() {
    let grid = generate_guid_grid(3, 2);
    let pieces = pieces_from_grid(&grid);

    let (solved, trace) = solve_puzzle_with_trace(pieces, 7).expect("puzzle should solve");

    assert_grid_has_matching_neighbors(&solved);
    assert!(matches!(trace.steps[0].action, TraceAction::Started));
    assert_eq!(trace.steps[0].polyominos.len(), 6);
    assert_eq!(
        trace
            .last_step()
            .expect("trace should have a final step")
            .polyominos
            .len(),
        1
    );
    assert!(
        trace
            .steps
            .iter()
            .any(|step| matches!(step.action, TraceAction::Joined { .. }))
    );
}
