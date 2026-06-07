use std::collections::HashMap;
use std::fmt::{self, Debug, Display};
use std::time::Duration;

use iced::Font;
use iced::mouse;
use iced::time;
use iced::widget::image::Handle;
use iced::widget::{
    button, canvas, checkbox, column, container, pick_list, row, slider, text, text_input,
};
use iced::{
    Color, Element, Fill, Point as CanvasPoint, Rectangle, Renderer, Size, Subscription, Task,
    Theme, Vector,
};
use jigsaw_simulation::{
    Direction, FirstAgainstRestPickingStrategy, Piece, PuzzleSolver, RandomPickingStrategy,
    SolveStep, TraceAction, TracePolyomino, generate_guid_grid, pieces_from_grid,
};

const FAST_AUTOPLAY_INTERVAL: Duration = Duration::from_millis(16);
const FAST_AUTOPLAY_STEPS_PER_TICK: usize = 64;
const MAX_STORED_STEPS: usize = 1_000;
const PUZZLE_IMAGE_SIZE: u32 = 1_024;
const THROTTLED_AUTOPLAY_INTERVAL: Duration = Duration::from_millis(250);

fn main() -> iced::Result {
    iced::application(TraceViewer::new, update, view)
        .default_font(Font::DEFAULT)
        .title(title)
        .subscription(subscription)
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
    StrategyChanged(SolverStrategy),
    ChooseImage,
    ImageSelected(Result<UploadedImage, String>),
    Generate,
    ToggleAutoPlay,
    ThrottleChanged(bool),
    AutoAdvance,
}

#[derive(Debug)]
struct TraceViewer {
    solver: Option<PuzzleSolver>,
    steps: Vec<SolveStep>,
    step_index: usize,
    first_stored_step_index: usize,
    image: PuzzleImage,
    image_tiles: HashMap<Piece, ImageTile>,
    width_input: String,
    height_input: String,
    strategy: SolverStrategy,
    status: String,
    is_playing: bool,
    is_throttled: bool,
    selected_image: Option<UploadedImage>,
}

impl TraceViewer {
    fn new() -> Self {
        let strategy = SolverStrategy::Random;
        let setup = start_solver(6, 6, strategy, None).expect("default trace should start");

        Self {
            solver: Some(setup.solver),
            steps: setup.steps,
            step_index: 0,
            first_stored_step_index: 0,
            image: setup.image,
            image_tiles: setup.image_tiles,
            width_input: String::from("6"),
            height_input: String::from("6"),
            strategy,
            status: String::from("6 x 6 puzzle ready"),
            is_playing: false,
            is_throttled: false,
            selected_image: None,
        }
    }

    fn current_step(&self) -> &SolveStep {
        &self.steps[self.step_index]
    }

    fn last_index(&self) -> usize {
        self.steps.len().saturating_sub(1)
    }

    fn absolute_step_index(&self) -> usize {
        self.first_stored_step_index + self.step_index
    }

    fn last_absolute_step_index(&self) -> usize {
        self.first_stored_step_index + self.last_index()
    }
}

fn update(viewer: &mut TraceViewer, message: Message) -> Task<Message> {
    match message {
        Message::Previous => viewer.step_index = viewer.step_index.saturating_sub(1),
        Message::Next => advance_to_next_step(viewer),
        Message::First => viewer.step_index = 0,
        Message::Last => viewer.step_index = viewer.last_index(),
        Message::StepChanged(step_index) => {
            viewer.step_index = (step_index as usize).min(viewer.last_index())
        }
        Message::WidthChanged(width) => viewer.width_input = width,
        Message::HeightChanged(height) => viewer.height_input = height,
        Message::StrategyChanged(strategy) => {
            viewer.strategy = strategy;
            viewer.is_playing = false;
            viewer.status = format!("strategy: {strategy}");
        }
        Message::ChooseImage => {
            viewer.is_playing = false;
            viewer.status = String::from("choosing image...");
            return Task::perform(choose_image_file(), Message::ImageSelected);
        }
        Message::ImageSelected(result) => {
            viewer.is_playing = false;
            match result {
                Ok(image) => {
                    viewer.status = format!(
                        "selected image: {} ({} x {})",
                        image.name, image.width, image.height
                    );
                    viewer.selected_image = Some(image);
                }
                Err(error) => viewer.status = error,
            }
        }
        Message::Generate => match requested_dimensions(viewer) {
            Ok((width, height)) => match start_solver(
                width,
                height,
                viewer.strategy,
                viewer.selected_image.as_ref(),
            ) {
                Ok(setup) => {
                    viewer.solver = Some(setup.solver);
                    viewer.steps = setup.steps;
                    viewer.step_index = 0;
                    viewer.first_stored_step_index = 0;
                    viewer.image = setup.image;
                    viewer.image_tiles = setup.image_tiles;
                    viewer.status =
                        format!("{width} x {height} puzzle ready with {}", viewer.strategy);
                    viewer.is_playing = false;
                }
                Err(error) => {
                    viewer.status = error;
                    viewer.is_playing = false;
                }
            },
            Err(message) => {
                viewer.status = message;
                viewer.is_playing = false;
            }
        },
        Message::ToggleAutoPlay => {
            viewer.is_playing = !viewer.is_playing;
            viewer.status = if viewer.is_playing {
                String::from("auto play running")
            } else {
                String::from("auto play stopped")
            };
        }
        Message::ThrottleChanged(is_throttled) => {
            viewer.is_throttled = is_throttled;
            viewer.status = if is_throttled {
                String::from("auto play throttled")
            } else {
                String::from("auto play unthrottled")
            };
        }
        Message::AutoAdvance => {
            if viewer.is_playing {
                advance_autoplay(viewer);
            }
        }
    }

    Task::none()
}

fn subscription(viewer: &TraceViewer) -> Subscription<Message> {
    if !viewer.is_playing {
        return Subscription::none();
    }

    let interval = if viewer.is_throttled {
        THROTTLED_AUTOPLAY_INTERVAL
    } else {
        FAST_AUTOPLAY_INTERVAL
    };

    time::every(interval).map(|_| Message::AutoAdvance)
}

fn view(viewer: &TraceViewer) -> Element<'_, Message> {
    let current_step = viewer.current_step();
    let header = row![
        text("Jigsaw trace").size(28),
        text(format!(
            "step {} of {} executed",
            viewer.absolute_step_index(),
            viewer.last_absolute_step_index()
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
        text("Strategy"),
        pick_list(
            SolverStrategy::ALL,
            Some(viewer.strategy),
            Message::StrategyChanged
        )
        .width(190),
        button("Choose image").on_press(Message::ChooseImage),
        button("Generate").on_press(Message::Generate),
        text(&viewer.status),
    ]
    .spacing(10)
    .align_y(iced::Alignment::Center);

    let controls = row![
        button("First").on_press(Message::First),
        button("Previous").on_press(Message::Previous),
        button("Execute step").on_press(Message::Next),
        button(if viewer.is_playing {
            "Stop"
        } else {
            "Auto play"
        })
        .on_press(Message::ToggleAutoPlay),
        checkbox(viewer.is_throttled)
            .label("Throttle")
            .on_toggle(Message::ThrottleChanged),
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
        image: viewer.image.clone(),
        image_tiles: viewer.image_tiles.clone(),
    })
    .width(Fill)
    .height(Fill);

    container(column![header, generator, controls, stage].spacing(14))
        .padding(18)
        .width(Fill)
        .height(Fill)
        .into()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SolverStrategy {
    Random,
    FirstAgainstRest,
}

impl SolverStrategy {
    const ALL: [Self; 2] = [Self::Random, Self::FirstAgainstRest];
}

impl Display for SolverStrategy {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SolverStrategy::Random => formatter.write_str("Random pair"),
            SolverStrategy::FirstAgainstRest => formatter.write_str("First against rest"),
        }
    }
}

#[derive(Clone, Debug)]
struct TraceCanvas {
    step: SolveStep,
    image: PuzzleImage,
    image_tiles: HashMap<Piece, ImageTile>,
}

#[derive(Clone, Debug)]
struct PuzzleImage {
    handle: Handle,
    cols: usize,
    rows: usize,
}

#[derive(Clone, Debug)]
struct UploadedImage {
    name: String,
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

#[derive(Clone, Copy, Debug)]
struct ImageTile {
    col: usize,
    row: usize,
    clockwise_rotations: u8,
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
            draw_polyomino(
                &mut frame,
                entry.polyomino,
                entry.origin,
                entry.cell_size,
                &self.image,
                &self.image_tiles,
            );
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
    image: &PuzzleImage,
    image_tiles: &HashMap<Piece, ImageTile>,
) {
    polyomino.cells.iter().for_each(|cell| {
        let x = origin.x + cell.point.x as f32 * cell_size;
        let y = origin.y + cell.point.y as f32 * cell_size;
        let size = cell_size - 2.0;
        let rect = canvas::Path::rectangle(CanvasPoint::new(x, y), Size::new(size, size));

        if let Some(tile) = image_tiles.get(&cell.piece) {
            draw_image_tile(frame, image, *tile, CanvasPoint::new(x, y), size);
        } else {
            frame.fill(&rect, Color::from_rgb8(202, 206, 211));
        }

        draw_side_colors(frame, &cell.piece, CanvasPoint::new(x, y), size);

        frame.stroke(
            &rect,
            canvas::Stroke::default()
                .with_color(Color::from_rgb8(42, 48, 58))
                .with_width(1.0),
        );
    });
}

fn draw_image_tile(
    frame: &mut canvas::Frame,
    image: &PuzzleImage,
    tile: ImageTile,
    origin: CanvasPoint,
    size: f32,
) {
    let clip = Rectangle::new(origin, Size::new(size, size));
    let full_width = image.cols as f32 * size;
    let full_height = image.rows as f32 * size;
    let full_origin = CanvasPoint::new(
        origin.x - tile.col as f32 * size,
        origin.y - tile.row as f32 * size,
    );

    frame.with_clip(clip, |frame| {
        frame.with_save(|frame| {
            let center = Vector::new(origin.x + size / 2.0, origin.y + size / 2.0);
            frame.translate(center);
            frame.rotate(std::f32::consts::FRAC_PI_2 * tile.clockwise_rotations as f32);
            frame.translate(Vector::new(-center.x, -center.y));
            frame.draw_image(
                Rectangle::new(full_origin, Size::new(full_width, full_height)),
                &image.handle,
            );
        });
    });
}

fn draw_side_colors(frame: &mut canvas::Frame, piece: &Piece, origin: CanvasPoint, size: f32) {
    let thickness = (size * 0.14).clamp(1.5, 5.0);

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
        let color = color_for_side(piece.side(direction));
        frame.fill(&edge, Color::from_rgba(color.r, color.g, color.b, 0.58));
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

fn advance_to_next_step(viewer: &mut TraceViewer) {
    if viewer.step_index + 1 < viewer.steps.len() {
        viewer.step_index += 1;
        return;
    }

    let Some(solver) = viewer.solver.as_mut() else {
        viewer.status = String::from("puzzle already solved");
        return;
    };

    match solver.next() {
        Some(Ok(step)) => {
            push_step(viewer, step);
            viewer.step_index = viewer.last_index();
            viewer.status = format!("executed step {}", viewer.absolute_step_index());
        }
        Some(Err(error)) => {
            viewer.solver = None;
            viewer.status = format!("could not solve puzzle: {error:?}");
        }
        None => {
            let status = match solver.solution() {
                Some(Ok(_)) => String::from("puzzle solved"),
                Some(Err(error)) => format!("could not solve puzzle: {error:?}"),
                None => String::from("solver stopped without a solution"),
            };
            viewer.solver = None;
            viewer.status = status;
        }
    }
}

fn push_step(viewer: &mut TraceViewer, step: SolveStep) {
    viewer.steps.push(step);

    if viewer.steps.len() <= MAX_STORED_STEPS {
        return;
    }

    viewer.steps.remove(0);
    viewer.first_stored_step_index += 1;
    viewer.step_index = viewer.step_index.saturating_sub(1);
}

fn advance_autoplay(viewer: &mut TraceViewer) {
    let steps_per_tick = if viewer.is_throttled {
        1
    } else {
        FAST_AUTOPLAY_STEPS_PER_TICK
    };

    (0..steps_per_tick).for_each(|_| {
        if viewer.solver.is_some() {
            advance_to_next_step(viewer);
        }
    });

    if viewer.solver.is_none() && viewer.step_index == viewer.last_index() {
        viewer.is_playing = false;
    }
}

#[derive(Debug)]
struct PuzzleSetup {
    solver: PuzzleSolver,
    steps: Vec<SolveStep>,
    image: PuzzleImage,
    image_tiles: HashMap<Piece, ImageTile>,
}

fn start_solver(
    width: usize,
    height: usize,
    strategy: SolverStrategy,
    uploaded_image: Option<&UploadedImage>,
) -> Result<PuzzleSetup, String> {
    let mut setup = build_puzzle(width, height, strategy, uploaded_image)
        .map_err(|error| format!("could not start puzzle solver: {error:?}"))?;
    let first_step = setup
        .solver
        .next()
        .ok_or_else(|| String::from("solver did not produce an initial step"))?
        .map_err(|error| format!("could not start puzzle solver: {error:?}"))?;
    setup.steps.push(first_step);

    Ok(setup)
}

fn build_puzzle(
    width: usize,
    height: usize,
    strategy: SolverStrategy,
    uploaded_image: Option<&UploadedImage>,
) -> Result<PuzzleSetup, jigsaw_simulation::PuzzleError> {
    let grid = generate_guid_grid(width, height);
    let image_tiles = image_tile_lookup(&grid);
    let mut pieces = pieces_from_grid(&grid);
    let mut rng = ViewerRng::new((width as u64) << 32 | height as u64);

    pieces.iter_mut().for_each(|piece| {
        *piece = (0..rng.next_index(4)).fold(piece.clone(), |piece, _| piece.rotate_clockwise())
    });

    (1..pieces.len()).rev().for_each(|index| {
        let swap_index = rng.next_index(index + 1);
        pieces.swap(index, swap_index);
    });

    let solver = match strategy {
        SolverStrategy::Random => {
            PuzzleSolver::with_picking_strategy(pieces, RandomPickingStrategy::new(9))
        }
        SolverStrategy::FirstAgainstRest => {
            PuzzleSolver::with_picking_strategy(pieces, FirstAgainstRestPickingStrategy::new())
        }
    }?;

    Ok(PuzzleSetup {
        solver,
        steps: Vec::new(),
        image: PuzzleImage {
            handle: uploaded_image
                .map(|image| uploaded_puzzle_image(image, width, height))
                .unwrap_or_else(generated_puzzle_image),
            cols: width,
            rows: height,
        },
        image_tiles,
    })
}

fn image_tile_lookup(grid: &[Vec<Piece>]) -> HashMap<Piece, ImageTile> {
    grid.iter()
        .enumerate()
        .flat_map(|(row, pieces)| {
            pieces.iter().enumerate().flat_map(move |(col, piece)| {
                (0..4).scan(piece.clone(), move |piece, clockwise_rotations| {
                    let rotated = piece.clone();
                    *piece = piece.rotate_clockwise();
                    Some((
                        rotated,
                        ImageTile {
                            col,
                            row,
                            clockwise_rotations,
                        },
                    ))
                })
            })
        })
        .collect()
}

fn generated_puzzle_image() -> Handle {
    let width = PUZZLE_IMAGE_SIZE;
    let height = PUZZLE_IMAGE_SIZE;
    let pixels = (0..height)
        .flat_map(|y| (0..width).flat_map(move |x| landscape_pixel(x, y, width, height)))
        .collect::<Vec<_>>();

    Handle::from_rgba(width, height, pixels)
}

fn uploaded_puzzle_image(uploaded: &UploadedImage, cols: usize, rows: usize) -> Handle {
    let source =
        image::RgbaImage::from_raw(uploaded.width, uploaded.height, uploaded.pixels.clone())
            .expect("uploaded image dimensions should match decoded RGBA pixels");
    let (crop_x, crop_y, crop_width, crop_height) =
        center_crop_rect(uploaded.width, uploaded.height, cols, rows);
    let cropped =
        image::imageops::crop_imm(&source, crop_x, crop_y, crop_width, crop_height).to_image();
    let (target_width, target_height) = fitted_image_size(cols, rows);
    let resized = image::imageops::resize(
        &cropped,
        target_width,
        target_height,
        image::imageops::FilterType::Triangle,
    );

    Handle::from_rgba(target_width, target_height, resized.into_raw())
}

fn center_crop_rect(
    source_width: u32,
    source_height: u32,
    cols: usize,
    rows: usize,
) -> (u32, u32, u32, u32) {
    let source_ratio = source_width as f32 / source_height as f32;
    let target_ratio = cols as f32 / rows as f32;

    if source_ratio > target_ratio {
        let crop_width =
            ((source_height as f32 * target_ratio).round() as u32).clamp(1, source_width);
        (
            (source_width - crop_width) / 2,
            0,
            crop_width,
            source_height,
        )
    } else {
        let crop_height =
            ((source_width as f32 / target_ratio).round() as u32).clamp(1, source_height);
        (
            0,
            (source_height - crop_height) / 2,
            source_width,
            crop_height,
        )
    }
}

fn fitted_image_size(cols: usize, rows: usize) -> (u32, u32) {
    if cols >= rows {
        (
            PUZZLE_IMAGE_SIZE,
            ((PUZZLE_IMAGE_SIZE as f32 * rows as f32 / cols as f32).round() as u32).max(1),
        )
    } else {
        (
            ((PUZZLE_IMAGE_SIZE as f32 * cols as f32 / rows as f32).round() as u32).max(1),
            PUZZLE_IMAGE_SIZE,
        )
    }
}

#[cfg(target_arch = "wasm32")]
fn decode_uploaded_image(name: String, bytes: Vec<u8>) -> Result<UploadedImage, String> {
    let decoded = image::load_from_memory(&bytes)
        .map_err(|error| format!("could not decode selected image: {error}"))?;
    let rgba = decoded.to_rgba8();
    let (width, height) = rgba.dimensions();

    Ok(UploadedImage {
        name,
        width,
        height,
        pixels: rgba.into_raw(),
    })
}

#[cfg(target_arch = "wasm32")]
async fn choose_image_file() -> Result<UploadedImage, String> {
    use futures_channel::oneshot;
    use js_sys::Uint8Array;
    use wasm_bindgen::JsCast;
    use wasm_bindgen::closure::Closure;
    use wasm_bindgen_futures::JsFuture;
    use web_sys::{HtmlElement, HtmlInputElement};

    let window = web_sys::window().ok_or_else(|| String::from("browser window not available"))?;
    let document = window
        .document()
        .ok_or_else(|| String::from("browser document not available"))?;
    let input = document
        .create_element("input")
        .map_err(|_| String::from("could not create image picker"))?
        .dyn_into::<HtmlInputElement>()
        .map_err(|_| String::from("could not prepare image picker"))?;

    input.set_type("file");
    input.set_accept("image/png,image/jpeg,image/*");

    let input_element = input
        .clone()
        .dyn_into::<HtmlElement>()
        .map_err(|_| String::from("could not hide image picker"))?;
    input_element.set_hidden(true);

    if let Some(body) = document.body() {
        body.append_child(&input)
            .map_err(|_| String::from("could not attach image picker"))?;
    }

    let (sender, receiver) = oneshot::channel::<Result<web_sys::File, String>>();
    let input_for_change = input.clone();
    let on_change = Closure::<dyn FnMut(web_sys::Event)>::once(move |_| {
        let selected_file = input_for_change
            .files()
            .and_then(|files| files.item(0))
            .ok_or_else(|| String::from("no image selected"));
        let _ = sender.send(selected_file);
    });

    input
        .add_event_listener_with_callback("change", on_change.as_ref().unchecked_ref())
        .map_err(|_| String::from("could not listen for selected image"))?;
    on_change.forget();

    input_element.click();

    let file = receiver
        .await
        .map_err(|_| String::from("image picker was cancelled"))??;
    let name = file.name();
    let blob: web_sys::Blob = file.unchecked_into();
    let array_buffer = JsFuture::from(blob.array_buffer())
        .await
        .map_err(|_| String::from("could not read selected image"))?;
    let bytes = Uint8Array::new(&array_buffer).to_vec();

    if let Some(parent) = input.parent_node() {
        let _ = parent.remove_child(&input);
    }

    decode_uploaded_image(name, bytes)
}

#[cfg(not(target_arch = "wasm32"))]
async fn choose_image_file() -> Result<UploadedImage, String> {
    Err(String::from(
        "image upload is available in the web viewer for now",
    ))
}

fn landscape_pixel(x: u32, y: u32, width: u32, height: u32) -> [u8; 4] {
    let fx = x as f32 / (width - 1) as f32;
    let fy = y as f32 / (height - 1) as f32;
    let texture = (((x.wrapping_mul(37) ^ y.wrapping_mul(19)) & 0xff) as f32 / 255.0 - 0.5) * 10.0;

    let (mut r, mut g, mut b) = if fy < 0.48 {
        let t = fy / 0.48;
        (42.0 + 72.0 * t, 118.0 + 78.0 * t, 190.0 + 40.0 * t)
    } else if fy < mountain_height(fx, 0.50, 0.10) {
        let shade = (1.0 - fy).clamp(0.0, 1.0);
        (
            62.0 + 50.0 * shade,
            82.0 + 48.0 * shade,
            88.0 + 42.0 * shade,
        )
    } else if fy < mountain_height(fx, 0.62, 0.08) {
        let shade = (1.0 - fy).clamp(0.0, 1.0);
        (
            46.0 + 38.0 * shade,
            92.0 + 62.0 * shade,
            82.0 + 36.0 * shade,
        )
    } else {
        let wave = ((fx * 34.0).sin() + (fy * 48.0).cos()) * 9.0;
        (32.0 + wave, 104.0 + wave, 142.0 + wave * 0.6)
    };

    let sun_dx = fx - 0.76;
    let sun_dy = fy - 0.22;
    let sun = (1.0 - ((sun_dx * sun_dx + sun_dy * sun_dy).sqrt() / 0.18)).clamp(0.0, 1.0);
    r += 130.0 * sun;
    g += 88.0 * sun;
    b += 18.0 * sun;

    let foreground = (fy - 0.72).max(0.0) / 0.28;
    r += foreground * (42.0 + texture);
    g += foreground * (34.0 + texture);
    b += foreground * (18.0 + texture);

    [
        (r + texture).clamp(0.0, 255.0) as u8,
        (g + texture).clamp(0.0, 255.0) as u8,
        (b + texture).clamp(0.0, 255.0) as u8,
        255,
    ]
}

fn mountain_height(x: f32, base: f32, amplitude: f32) -> f32 {
    base + amplitude * (x * std::f32::consts::TAU * 1.6).sin()
        + amplitude * 0.55 * (x * std::f32::consts::TAU * 4.3).cos()
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
