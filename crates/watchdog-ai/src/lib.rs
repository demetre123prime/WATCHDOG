use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use image::{DynamicImage, ImageFormat, RgbaImage};
use serde_json::{json, Value};
use std::io::Cursor;
use std::time::Duration;
use watchdog_core::CapturedFrame;

/// Ask the local Ollama service. No hosted API is used by this crate.
pub fn ask_local(
    endpoint: &str,
    model: &str,
    question: &str,
    frame: Option<&CapturedFrame>,
) -> Result<String, String> {
    if model.trim().is_empty() {
        return Err("Enter a model name.".into());
    }
    if question.trim().is_empty() {
        return Err("Enter a question first.".into());
    }

    let mut message = json!({
        "role": "user",
        "content": question.trim()
    });

    if let Some(frame) = frame {
        if !frame.is_valid() {
            return Err("The latest screen frame is invalid.".into());
        }
        let image = RgbaImage::from_raw(frame.width, frame.height, frame.rgba.clone())
            .ok_or_else(|| "Could not decode the latest screen frame.".to_owned())?;
        let mut cursor = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(image)
            .write_to(&mut cursor, ImageFormat::Png)
            .map_err(|error| error.to_string())?;
        message["images"] = json!([BASE64.encode(cursor.into_inner())]);
    }

    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(180))
        .build()
        .map_err(|error| error.to_string())?;

    let response: Value = client
        .post(endpoint)
        .json(&json!({
            "model": model.trim(),
            "messages": [message],
            "stream": false
        }))
        .send()
        .map_err(|error| {
            if error.is_connect() {
                format!("Cannot reach {endpoint}. Start Ollama and pull the selected model.")
            } else {
                error.to_string()
            }
        })?
        .error_for_status()
        .map_err(|error| format!("Ollama returned an HTTP error: {error}"))?
        .json()
        .map_err(|error| format!("Could not parse Ollama's response: {error}"))?;

    response
        .pointer("/message/content")
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| "Ollama returned no message text. Check the model name and vision support.".into())
}
