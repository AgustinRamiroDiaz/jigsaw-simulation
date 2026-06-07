use std::fmt::Debug;

use iced::mouse;
use iced::widget::{button, canvas, column, container, row, slider, text, text_input};
use iced::{Color, Element, Fill, Point as CanvasPoint, Rectangle, Renderer, Size, Theme};
use jigsaw_simulation::{
    Direction, Piece, SolveStep, SolveTrace, TraceAction, TracePolyomino, generate_guid_grid,
    pieces_from_grid, solve_puzzle_with_trace,
};

fn main() -> iced::Result {
    iced::application(TraceViewer::new, update, view)
        .title(title)
        .window_size(Size::new(1180.0, 760.0))
        .antialiasing(true)
        .run()
}

fn title(_viewer: &TraceViewer) -> String {
    String::from("Jigsaw Trace Viewer")
}

#[derive(Clone, Debug)]
enum Message {
    Previous,
    Next,
    First,
    Last,
    StepChanged(u32),
    WidthChanged(String),
    HeightChanged(String),
    Generate,
}

#[derive(Debug)]
struct TraceViewer {
    trace: SolveTrace,
    step_index: usize,
    width_input: String,
    height_input: String,
    status: String,
}

impl TraceViewer {
    fn new() -> Self {
        Self {
            trace: build_trace(6, 6).expect("default trace should solve"),
            step_index: 0,
            width_input: String::from("6"),
            height_input: String::from("6"),
            status: String::from("6 x 6 puzzle"),
        }
    }

    fn current_step(&self) -> &SolveStep {
        &self.trace.steps[self.step_index]
    }

    fn last_index(&self) -> usize {
        self.trace.steps.len().saturating_sub(1)
    }
}

fn update(viewer: &mut TraceViewer, message: Message) {
    match message {
        Message::Previous => viewer.step_index = viewer.step_index.saturating_sub(1),
        Message::Next => viewer.step_index = (viewer.step_index + 1).min(viewer.last_index()),
        Message::First => viewer.step_index = 0,
        Message::Last => viewer.step_index = viewer.last_index(),
        Message::StepChanged(step_index) => {
            viewer.step_index = (step_index as usize).min(viewer.last_index())
        }
        Message::WidthChanged(width) => viewer.width_input = width,
        Message::HeightChanged(height) => viewer.height_input = height,
        Message::Generate => match requested_dimensions(viewer) {
            Ok((width, height)) => match build_trace(width, height) {
                Ok(trace) => {
                    viewer.trace = trace;
                    viewer.step_index = 0;
                    viewer.status = format!("{width} x {height} puzzle");
                }
                Err(error) => {
                    viewer.status = format!("could not solve puzzle: {error:?}");
                }
            },
            Err(message) => viewer.status = message,
        },
    }
}

fn view(viewer: &TraceViewer) -> Element<'_, Message> {
    let current_step = viewer.current_step();
    let header = row![
        text("Jigsaw trace").size(28),
        text(format!(
            "step {} of {}",
            viewer.step_index,
            viewer.last_index()
        ))
        .size(18),
        text(action_label(&current_step.action)).size(18),
    ]
    .spacing(18)
    .align_y(iced::Alignment::Center);

    let generator = row![
        text("Width"),
        text_input("width", &viewer.width_input)
            .on_input(Message::WidthChanged)
            .width(80),
        text("Height"),
        text_input("height", &viewer.height_input)
            .on_input(Message::HeightChanged)
            .width(80),
        button("Generate").on_press(Message::Generate),
        text(&viewer.status),
    ]
    .spacing(10)
    .align_y(iced::Alignment::Center);

    let controls = row![
        button("First").on_press(Message::First),
        button("Previous").on_press(Message::Previous),
        button("Next").on_press(Message::Next),
        button("Last").on_press(Message::Last),
        slider(
            0..=viewer.last_index() as u32,
            viewer.step_index as u32,
            Message::StepChanged
        )
        .width(Fill),
    ]
    .spacing(10)
    .align_y(iced::Alignment::Center);

    let stage = canvas(TraceCanvas {
        step: current_step.clone(),
    })
    .width(Fill)
    .height(Fill);

    container(column![header, generator, controls, stage].spacing(14))
        .padding(18)
        .width(Fill)
        .height(Fill)
        .into()
}

#[derive(Clone, Debug)]
struct TraceCanvas {
    step: SolveStep,
}

impl<Message> canvas::Program<Message> for TraceCanvas {
    type State = ();

    fn draw(
        &self,
        _state: &(),
        renderer: &Renderer,
        _theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let background = canvas::Path::rectangle(CanvasPoint::ORIGIN, bounds.size());
        frame.fill(&background, Color::from_rgb8(245, 247, 250));

        let layout = layout_polyominos(&self.step.polyominos, bounds.width);

        layout.iter().for_each(|entry| {
            draw_polyomino(&mut frame, entry.polyomino, entry.origin, entry.cell_size);
        });

        vec![frame.into_geometry()]
    }
}

#[derive(Clone, Debug)]
struct PolyominoLayout<'a> {
    polyomino: &'a TracePolyomino,
    origin: CanvasPoint,
    cell_size: f32,
}

fn layout_polyominos(polyominos: &[TracePolyomino], width: f32) -> Vec<PolyominoLayout<'_>> {
    let margin = 24.0;
    let gap = 20.0;
    let cell_size = if polyominos.len() > 30 { 12.0 } else { 24.0 };

    polyominos
        .iter()
        .scan((margin, margin, 0.0), |(x, y, row_height), polyomino| {
            let (poly_width, poly_height) = polyomino_size(polyomino, cell_size);

            if *x + poly_width > width - margin && *x > margin {
                *x = margin;
                *y += *row_height + gap;
                *row_height = 0.0;
            }

            let origin = CanvasPoint::new(*x, *y);
            *x += poly_width + gap;
            *row_height = row_height.max(poly_height);

            Some(PolyominoLayout {
                polyomino,
                origin,
                cell_size,
            })
        })
        .collect()
}

fn draw_polyomino(
    frame: &mut canvas::Frame,
    polyomino: &TracePolyomino,
    origin: CanvasPoint,
    cell_size: f32,
) {
    polyomino.cells.iter().for_each(|cell| {
        let x = origin.x + cell.point.x as f32 * cell_size;
        let y = origin.y + cell.point.y as f32 * cell_size;
        let size = cell_size - 2.0;
        let rect = canvas::Path::rectangle(CanvasPoint::new(x, y), Size::new(size, size));
        frame.fill(&rect, Color::from_rgb8(202, 206, 211));

        draw_side_colors(frame, &cell.piece, CanvasPoint::new(x, y), size);

        frame.stroke(
            &rect,
            canvas::Stroke::default()
                .with_color(Color::from_rgb8(42, 48, 58))
                .with_width(1.0),
        );
    });
}

fn draw_side_colors(frame: &mut canvas::Frame, piece: &Piece, origin: CanvasPoint, size: f32) {
    let thickness = (size * 0.22).clamp(2.0, 7.0);

    [
        (Direction::Top, origin, Size::new(size, thickness)),
        (
            Direction::Right,
            CanvasPoint::new(origin.x + size - thickness, origin.y),
            Size::new(thickness, size),
        ),
        (
            Direction::Bottom,
            CanvasPoint::new(origin.x, origin.y + size - thickness),
            Size::new(size, thickness),
        ),
        (Direction::Left, origin, Size::new(thickness, size)),
    ]
    .into_iter()
    .for_each(|(direction, origin, size)| {
        let edge = canvas::Path::rectangle(origin, size);
        frame.fill(&edge, color_for_side(piece.side(direction)));
    });
}

fn polyomino_size(polyomino: &TracePolyomino, cell_size: f32) -> (f32, f32) {
    let max_x = polyomino
        .cells
        .iter()
        .map(|cell| cell.point.x)
        .max()
        .unwrap_or(0);
    let max_y = polyomino
        .cells
        .iter()
        .map(|cell| cell.point.y)
        .max()
        .unwrap_or(0);

    (
        (max_x + 1) as f32 * cell_size,
        (max_y + 1) as f32 * cell_size,
    )
}

fn color_for_side(side: &impl Debug) -> Color {
    let hash = format!("{side:?}").bytes().fold(0_u32, |hash, byte| {
        hash.wrapping_mul(31).wrapping_add(byte as u32)
    });

    hsl_to_rgb((hash % 360) as f32, 0.68, 0.56)
}

fn hsl_to_rgb(hue: f32, saturation: f32, lightness: f32) -> Color {
    let chroma = (1.0 - (2.0 * lightness - 1.0).abs()) * saturation;
    let hue_prime = hue / 60.0;
    let x = chroma * (1.0 - (hue_prime % 2.0 - 1.0).abs());
    let (r1, g1, b1) = match hue_prime as u8 {
        0 => (chroma, x, 0.0),
        1 => (x, chroma, 0.0),
        2 => (0.0, chroma, x),
        3 => (0.0, x, chroma),
        4 => (x, 0.0, chroma),
        _ => (chroma, 0.0, x),
    };
    let m = lightness - chroma / 2.0;

    Color::from_rgb(r1 + m, g1 + m, b1 + m)
}

fn action_label(action: &TraceAction) -> String {
    match action {
        TraceAction::Started => String::from("started"),
        TraceAction::Joined {
            first_index,
            second_index,
        } => format!("joined {first_index} + {second_index}"),
        TraceAction::Rejected {
            first_index,
            second_index,
        } => format!("rejected {first_index} + {second_index}"),
        TraceAction::FallbackJoined {
            first_index,
            second_index,
        } => format!("fallback joined {first_index} + {second_index}"),
    }
}

fn requested_dimensions(viewer: &TraceViewer) -> Result<(usize, usize), String> {
    let width: usize = viewer
        .width_input
        .trim()
        .parse()
        .map_err(|_| String::from("width must be a positive number"))?;
    let height: usize = viewer
        .height_input
        .trim()
        .parse()
        .map_err(|_| String::from("height must be a positive number"))?;

    match (width, height) {
        (0, _) => Err(String::from("width must be at least 1")),
        (_, 0) => Err(String::from("height must be at least 1")),
        (width, height) if width.saturating_mul(height) > 10000 => {
            Err(String::from("keep puzzles at 10000 pieces or fewer"))
        }
        dimensions => Ok(dimensions),
    }
}

fn build_trace(width: usize, height: usize) -> Result<SolveTrace, jigsaw_simulation::PuzzleError> {
    let grid = generate_guid_grid(width, height);
    let mut pieces = pieces_from_grid(&grid);
    let mut rng = ViewerRng::new((width as u64) << 32 | height as u64);

    pieces.iter_mut().for_each(|piece| {
        *piece = (0..rng.next_index(4)).fold(piece.clone(), |piece, _| piece.rotate_clockwise())
    });

    (1..pieces.len()).rev().for_each(|index| {
        let swap_index = rng.next_index(index + 1);
        pieces.swap(index, swap_index);
    });

    solve_puzzle_with_trace(pieces, 9).map(|(_, trace)| trace)
}

#[derive(Debug)]
struct ViewerRng {
    state: u64,
}

impl ViewerRng {
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
