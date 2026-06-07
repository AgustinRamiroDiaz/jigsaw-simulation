#[derive(Clone, Debug)]
pub(crate) struct UploadedImage {
    pub(crate) name: String,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) pixels: Vec<u8>,
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
pub(crate) async fn choose_image_file() -> Result<UploadedImage, String> {
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
pub(crate) async fn choose_image_file() -> Result<UploadedImage, String> {
    Err(String::from(
        "image upload is available in the web viewer for now",
    ))
}
