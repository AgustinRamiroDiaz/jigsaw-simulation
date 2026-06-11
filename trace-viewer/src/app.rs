use std::collections::HashMap;

use eframe::egui::{self, ColorImage, ComboBox, Context, TextureHandle, TextureOptions};
#[cfg(target_arch = "wasm32")]
use futures_channel::oneshot;
use jigsaw_simulation::{Piece, SolveStep, TraceAction};
use web_time::{Duration, Instant};

use crate::canvas::draw_trace_canvas;
use crate::image_upload::UploadedImage;
use crate::image_upload::capture_image_file;
use crate::image_upload::choose_image_file;
use crate::puzzle::{ImageTile, PuzzleImage, TraceSolver, start_solver};
use crate::strategy::SolverStrategy;

const FAST_AUTOPLAY_INTERVAL: Duration = Duration::from_millis(16);
const FAST_AUTOPLAY_STEPS_PER_TICK: usize = 64;
const MAX_STORED_STEPS: usize = 1_000;
const THROTTLED_AUTOPLAY_INTERVAL: Duration = Duration::from_millis(250);

#[cfg(not(target_arch = "wasm32"))]
pub(crate) const TITLE: &str = "Jigsaw Trace Viewer";

pub(crate) struct TraceViewer {
    solver: Option<Box<dyn TraceSolver>>,
    steps: Vec<SolveStep>,
    step_index: usize,
    first_stored_step_index: usize,
    image: PuzzleImage,
    image_texture: Option<TextureHandle>,
    image_tiles: HashMap<Piece, ImageTile>,
    width_input: String,
    height_input: String,
    strategy: SolverStrategy,
    status: String,
    is_playing: bool,
    is_throttled: bool,
    selected_image: Option<UploadedImage>,
    #[cfg(target_arch = "wasm32")]
    image_upload: Option<oneshot::Receiver<Result<UploadedImage, String>>>,
    last_autoplay_tick: Instant,
}

impl TraceViewer {
    pub(crate) fn new(_creation_context: &eframe::CreationContext<'_>) -> Self {
        let strategy = SolverStrategy::Random;
        let setup = start_solver(6, 6, strategy, None).expect("default trace should start");

        Self {
            solver: Some(setup.solver),
            steps: setup.steps,
            step_index: 0,
            first_stored_step_index: 0,
            image: setup.image,
            image_texture: None,
            image_tiles: setup.image_tiles,
            width_input: String::from("6"),
            height_input: String::from("6"),
            strategy,
            status: String::from("6 x 6 puzzle ready"),
            is_playing: false,
            is_throttled: false,
            selected_image: None,
            #[cfg(target_arch = "wasm32")]
            image_upload: None,
            last_autoplay_tick: Instant::now(),
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

    fn ensure_texture(&mut self, ctx: &Context) {
        if self.image_texture.is_some() {
            return;
        }

        let image = ColorImage::from_rgba_unmultiplied(
            [self.image.width as usize, self.image.height as usize],
            &self.image.pixels,
        );
        self.image_texture = Some(ctx.load_texture("puzzle-image", image, TextureOptions::LINEAR));
    }

    fn generate(&mut self) {
        match requested_dimensions(self) {
            Ok((width, height)) => {
                match start_solver(width, height, self.strategy, self.selected_image.as_ref()) {
                    Ok(setup) => {
                        self.solver = Some(setup.solver);
                        self.steps = setup.steps;
                        self.step_index = 0;
                        self.first_stored_step_index = 0;
                        self.image = setup.image;
                        self.image_texture = None;
                        self.image_tiles = setup.image_tiles;
                        self.status =
                            format!("{width} x {height} puzzle ready with {}", self.strategy);
                        self.is_playing = false;
                    }
                    Err(error) => {
                        self.status = error;
                        self.is_playing = false;
                    }
                }
            }
            Err(message) => {
                self.status = message;
                self.is_playing = false;
            }
        }
    }

    fn choose_image(&mut self) {
        self.is_playing = false;
        self.status = String::from("choosing image...");

        #[cfg(target_arch = "wasm32")]
        {
            let (sender, receiver) = oneshot::channel();
            wasm_bindgen_futures::spawn_local(async move {
                let _ = sender.send(choose_image_file().await);
            });
            self.image_upload = Some(receiver);
        }

        #[cfg(not(target_arch = "wasm32"))]
        self.receive_selected_image(choose_image_file());
    }

    fn capture_image(&mut self) {
        self.is_playing = false;
        self.status = String::from("opening camera...");

        #[cfg(target_arch = "wasm32")]
        {
            let (sender, receiver) = oneshot::channel();
            wasm_bindgen_futures::spawn_local(async move {
                let _ = sender.send(capture_image_file().await);
            });
            self.image_upload = Some(receiver);
        }

        #[cfg(not(target_arch = "wasm32"))]
        self.receive_selected_image(capture_image_file());
    }

    fn receive_selected_image(&mut self, result: Result<UploadedImage, String>) {
        match result {
            Ok(image) => {
                self.status = format!(
                    "selected image: {} ({} x {})",
                    image.name, image.width, image.height
                );
                self.selected_image = Some(image);
            }
            Err(error) => self.status = error,
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn poll_image_upload(&mut self, ctx: &Context) {
        let Some(receiver) = self.image_upload.as_mut() else {
            return;
        };

        match receiver.try_recv() {
            Ok(Some(result)) => {
                self.image_upload = None;
                self.receive_selected_image(result);
            }
            Ok(None) => ctx.request_repaint_after(Duration::from_millis(100)),
            Err(_) => {
                self.image_upload = None;
                self.status = String::from("image picker was cancelled");
            }
        }
    }

    fn autoplay_interval(&self) -> Duration {
        if self.is_throttled {
            THROTTLED_AUTOPLAY_INTERVAL
        } else {
            FAST_AUTOPLAY_INTERVAL
        }
    }

    fn update_autoplay(&mut self, ctx: &Context) {
        if !self.is_playing {
            return;
        }

        let interval = self.autoplay_interval();

        if self.last_autoplay_tick.elapsed() >= interval {
            advance_autoplay(self);
            self.last_autoplay_tick = Instant::now();
        }

        ctx.request_repaint_after(interval);
    }
}

impl eframe::App for TraceViewer {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();

        self.update_autoplay(&ctx);
        #[cfg(target_arch = "wasm32")]
        self.poll_image_upload(&ctx);
        self.ensure_texture(&ctx);

        egui::Panel::left("trace_config")
            .resizable(false)
            .exact_size(260.0)
            .show_inside(ui, |ui| {
                ui.add_space(10.0);
                ui.heading("Jigsaw trace");
                ui.add_space(10.0);

                ui.label("Puzzle size");
                ui.horizontal(|ui| {
                    ui.label("Width");
                    let width = ui.add_sized(
                        [72.0, 24.0],
                        egui::TextEdit::singleline(&mut self.width_input),
                    );

                    ui.label("Height");
                    let height = ui.add_sized(
                        [72.0, 24.0],
                        egui::TextEdit::singleline(&mut self.height_input),
                    );

                    if (width.lost_focus() || height.lost_focus())
                        && ui.input(|input| input.key_pressed(egui::Key::Enter))
                    {
                        self.generate();
                    }
                });

                ui.add_space(10.0);
                ui.label("Strategy");
                let previous_strategy = self.strategy;
                ComboBox::from_id_salt("strategy")
                    .selected_text(self.strategy.to_string())
                    .width(ui.available_width())
                    .show_ui(ui, |ui| {
                        SolverStrategy::ALL.into_iter().for_each(|strategy| {
                            ui.selectable_value(&mut self.strategy, strategy, strategy.to_string());
                        });
                    });

                if self.strategy != previous_strategy {
                    self.is_playing = false;
                    self.status = format!("strategy: {}", self.strategy);
                }

                ui.add_space(10.0);
                if ui
                    .add_sized([ui.available_width(), 28.0], egui::Button::new("Generate"))
                    .clicked()
                {
                    self.generate();
                }

                ui.add_space(14.0);
                ui.separator();
                ui.add_space(10.0);
                ui.label("Image");
                if ui
                    .add_sized([ui.available_width(), 28.0], egui::Button::new("Upload photo"))
                    .clicked()
                {
                    self.choose_image();
                }
                if ui
                    .add_sized([ui.available_width(), 28.0], egui::Button::new("Take picture"))
                    .clicked()
                {
                    self.capture_image();
                }

                ui.add_space(14.0);
                ui.separator();
                ui.add_space(10.0);
                ui.label("Playback");
                ui.horizontal(|ui| {
                    if ui.button("First").clicked() {
                        self.step_index = 0;
                    }

                    if ui.button("Prev").clicked() {
                        self.step_index = self.step_index.saturating_sub(1);
                    }

                    if ui.button("Step").clicked() {
                        advance_to_next_step(self);
                    }
                });

                ui.horizontal(|ui| {
                    let autoplay_label = if self.is_playing { "Stop" } else { "Auto" };
                    if ui.button(autoplay_label).clicked() {
                        self.is_playing = !self.is_playing;
                        self.last_autoplay_tick = Instant::now();
                        self.status = if self.is_playing {
                            String::from("auto play running")
                        } else {
                            String::from("auto play stopped")
                        };
                    }

                    if ui.button("Last").clicked() {
                        self.step_index = self.last_index();
                    }

                    if ui.checkbox(&mut self.is_throttled, "Throttle").changed() {
                        self.status = if self.is_throttled {
                            String::from("auto play throttled")
                        } else {
                            String::from("auto play unthrottled")
                        };
                    }
                });

                let last_index = self.last_index();
                ui.add(
                    egui::Slider::new(&mut self.step_index, 0..=last_index)
                        .show_value(false)
                        .clamping(egui::SliderClamping::Always),
                );

                ui.add_space(14.0);
                ui.separator();
                ui.add_space(10.0);
                ui.label("Status");
                ui.label(&self.status);
            });

        egui::CentralPanel::default().show_inside(ui, |ui| {
            ui.add_space(8.0);
            ui.horizontal_wrapped(|ui| {
                ui.label(format!(
                    "Step {} of {} executed",
                    self.absolute_step_index(),
                    self.last_absolute_step_index()
                ));
                ui.separator();
                ui.label(action_label(&self.current_step().action));
            });
            ui.add_space(8.0);
            ui.separator();

            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    draw_trace_canvas(
                        ui,
                        self.current_step(),
                        &self.image,
                        &self.image_tiles,
                        self.image_texture.as_ref(),
                    );
                });
        });
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
