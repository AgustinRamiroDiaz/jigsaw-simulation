mod app;
mod canvas;
mod image_upload;
mod puzzle;
mod rng;
mod strategy;

use app::{TraceViewer, subscription, title, update, view};
use iced::{Font, Size};

fn main() -> iced::Result {
    iced::application(TraceViewer::new, update, view)
        .default_font(Font::DEFAULT)
        .title(title)
        .subscription(subscription)
        .window_size(Size::new(1180.0, 760.0))
        .antialiasing(true)
        .run()
}
