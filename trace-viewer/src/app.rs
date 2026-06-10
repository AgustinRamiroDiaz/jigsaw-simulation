use std::collections::HashMap;
use std::time::Duration;

use iced::time;
use iced::widget::{
    button, canvas, checkbox, column, container, pick_list, row, slider, text, text_input,
};
use iced::{Element, Fill, Subscription, Task, event, keyboard};
use jigsaw_simulation::{Piece, SolveStep, TraceAction};

use crate::canvas::TraceCanvas;
use crate::image_upload::{UploadedImage, choose_image_file};
use crate::puzzle::{ImageTile, PuzzleImage, TraceSolver, start_solver};
use crate::strategy::SolverStrategy;

const FAST_AUTOPLAY_INTERVAL: Duration = Duration::from_millis(16);
const FAST_AUTOPLAY_STEPS_PER_TICK: usize = 64;
const MAX_STORED_STEPS: usize = 1_000;
const THROTTLED_AUTOPLAY_INTERVAL: Duration = Duration::from_millis(250);

pub(crate) fn title(_viewer: &TraceViewer) -> String {
    String::from("Jigsaw Trace Viewer")
}

#[derive(Clone, Debug)]
pub(crate) enum Message {
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
    FocusNext,
    FocusPrevious,
    ToggleAutoPlay,
    ThrottleChanged(bool),
    AutoAdvance,
}

pub(crate) struct TraceViewer {
    solver: Option<Box<dyn TraceSolver>>,
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
    pub(crate) fn new() -> Self {
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

pub(crate) fn update(viewer: &mut TraceViewer, message: Message) -> Task<Message> {
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
        Message::FocusNext => return iced::widget::operation::focus_next(),
        Message::FocusPrevious => return iced::widget::operation::focus_previous(),
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

pub(crate) fn subscription(viewer: &TraceViewer) -> Subscription<Message> {
    let keyboard = event::listen_with(tab_navigation);

    if !viewer.is_playing {
        return keyboard;
    }

    let interval = if viewer.is_throttled {
        THROTTLED_AUTOPLAY_INTERVAL
    } else {
        FAST_AUTOPLAY_INTERVAL
    };

    Subscription::batch([
        keyboard,
        time::every(interval).map(|_| Message::AutoAdvance),
    ])
}

pub(crate) fn view(viewer: &TraceViewer) -> Element<'_, Message> {
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
            .id("width-input")
            .on_input(Message::WidthChanged)
            .on_submit(Message::Generate)
            .width(80),
        text("Height"),
        text_input("height", &viewer.height_input)
            .id("height-input")
            .on_input(Message::HeightChanged)
            .on_submit(Message::Generate)
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

fn tab_navigation(
    event: iced::Event,
    _status: event::Status,
    _window: iced::window::Id,
) -> Option<Message> {
    match event {
        iced::Event::Keyboard(keyboard::Event::KeyPressed {
            key: keyboard::Key::Named(keyboard::key::Named::Tab),
            modifiers,
            ..
        }) if modifiers.shift() => Some(Message::FocusPrevious),
        iced::Event::Keyboard(keyboard::Event::KeyPressed {
            key: keyboard::Key::Named(keyboard::key::Named::Tab),
            ..
        }) => Some(Message::FocusNext),
        _ => None,
    }
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
