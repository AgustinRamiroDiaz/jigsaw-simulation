mod app;
mod canvas;
mod image_upload;
mod puzzle;
mod rng;
mod strategy;

use app::{TraceViewer, subscription, title, update, view};
use iced::{Font, Size};

fn main() -> iced::Result {
    init_wasm_diagnostics();

    iced::application(TraceViewer::new, update, view)
        .default_font(Font::DEFAULT)
        .title(title)
        .subscription(subscription)
        .window_size(Size::new(1180.0, 760.0))
        .antialiasing(true)
        .run()
}

#[cfg(target_arch = "wasm32")]
fn init_wasm_diagnostics() {
    console_error_panic_hook::set_once();
    let _ = console_log::init_with_level(log::Level::Debug);
}

#[cfg(not(target_arch = "wasm32"))]
fn init_wasm_diagnostics() {}
