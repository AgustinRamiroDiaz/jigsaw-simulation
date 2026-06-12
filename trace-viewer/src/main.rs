mod app;
mod canvas;
mod image_upload;
mod puzzle;
mod rng;
mod strategy;

#[cfg(target_arch = "wasm32")]
use app::TraceViewer;
#[cfg(not(target_arch = "wasm32"))]
use app::{TITLE, TraceViewer};

#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result {
    init_wasm_diagnostics();

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default().with_inner_size([1180.0, 760.0]),
        ..Default::default()
    };

    eframe::run_native(
        TITLE,
        options,
        Box::new(|creation_context| Ok(Box::new(TraceViewer::new(creation_context)))),
    )
}

#[cfg(target_arch = "wasm32")]
fn main() {
    init_wasm_diagnostics();
}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn start() {
    init_wasm_diagnostics();

    wasm_bindgen_futures::spawn_local(async {
        if let Err(error) = start_web().await {
            log::error!("could not start trace viewer: {error:?}");
        }
    });
}

#[cfg(target_arch = "wasm32")]
async fn start_web() -> Result<(), wasm_bindgen::JsValue> {
    use wasm_bindgen::JsCast;

    let window = web_sys::window().ok_or_else(|| "browser window not available")?;
    let document = window
        .document()
        .ok_or_else(|| "browser document not available")?;
    let body = document
        .body()
        .ok_or_else(|| "document body not available")?;

    let canvas = match document.get_element_by_id("trace-viewer-canvas") {
        Some(canvas) => canvas.dyn_into::<web_sys::HtmlCanvasElement>()?,
        None => {
            let canvas = document
                .create_element("canvas")?
                .dyn_into::<web_sys::HtmlCanvasElement>()?;
            canvas.set_id("trace-viewer-canvas");
            canvas.set_attribute("style", "width: 100vw; height: 100vh; display: block;")?;
            body.append_child(&canvas)?;
            canvas
        }
    };

    let runner = Box::leak(Box::new(eframe::WebRunner::new()));
    let web_options = eframe::WebOptions {
        renderer: eframe::Renderer::Glow,
        ..Default::default()
    };

    runner
        .start(
            canvas,
            web_options,
            Box::new(|creation_context| Ok(Box::new(TraceViewer::new(creation_context)))),
        )
        .await
}

#[cfg(target_arch = "wasm32")]
fn init_wasm_diagnostics() {
    console_error_panic_hook::set_once();
    let _ = console_log::init_with_level(log::Level::Debug);
}

#[cfg(not(target_arch = "wasm32"))]
fn init_wasm_diagnostics() {}
