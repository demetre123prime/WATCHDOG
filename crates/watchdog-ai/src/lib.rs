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


/// A one-step, explainable recommendation from a visual game-learning session.
#[derive(Clone, Debug, Default)]
pub struct GameCoachUpdate {
    pub game_name: String,
    pub current_goal: String,
    pub scene_summary: String,
    pub next_step: String,
    pub question: String,
    pub confidence: String,
    pub learning: String,
}

/// Analyze one current screen frame for a user-started game guide session.
/// The model receives only the current frame, the selected game name, and
/// player-provided/approved text context.
pub fn coach_game(
    endpoint: &str,
    model: &str,
    game_name: &str,
    frame: &CapturedFrame,
    player_context: &[String],
    approved_notes: &[String],
) -> Result<GameCoachUpdate, String> {
    if model.trim().is_empty() {
        return Err("Enter a local vision model name.".into());
    }
    if !frame.is_valid() {
        return Err("The current game frame is invalid.".into());
    }

    let image = RgbaImage::from_raw(frame.width, frame.height, frame.rgba.clone())
        .ok_or_else(|| "Could not decode the current game frame.".to_owned())?;
    let mut cursor = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(image)
        .write_to(&mut cursor, ImageFormat::Png)
        .map_err(|error| error.to_string())?;

    let player_context = if player_context.is_empty() {
        "No player explanations yet.".to_owned()
    } else {
        player_context.join("\\n")
    };
    let approved_notes = if approved_notes.is_empty() {
        "No saved notes for this game yet.".to_owned()
    } else {
        approved_notes.join("\\n")
    };

    let system_prompt = "You are WATCHDOG, a patient local game-learning coach. You are observing a game only because the player explicitly started a visible WATCHDOG session. Inspect the screenshot, identify visible game UI and likely objective, and help the player understand the game one step at a time. Ignore WATCHDOG's own overlay if it appears in the screenshot. Do not claim that you clicked or performed anything. Never control the game, click, type, or execute macros. Suggest only one clear, safe, reversible in-game action at a time, phrased as an instruction to the player (for example: 'Open the shop' or 'Try the throw button once'). If a mechanic, objective, or visible item is unclear, ask exactly one short question in the question field and wait for the player's answer rather than guessing. Use the player's prior explanations and approved notes as context, but distinguish them from what is actually visible. Do not ask for passwords, account details, private messages, or other sensitive information. Return only JSON with string fields: current_goal, scene_summary, next_step, question, confidence, learning. confidence must be low, medium, or high. learning should be a short useful fact only when the screenshot or player explanation supports it.";

    let user_prompt = format!(
        "Selected game: {game_name}\n\nPlayer explanations from this session:\n{player_context}\n\nPlayer-approved saved notes for this game:\n{approved_notes}\n\nStudy the attached current screenshot. Give the player the single best next step. If you need information, ask one short question and do not invent a game mechanic."
    );

    let message = json!({
        "role": "user",
        "content": user_prompt,
        "images": [BASE64.encode(cursor.into_inner())]
    });

    let url = format!("{}/api/chat", endpoint.trim().trim_end_matches('/'));
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(180))
        .build()
        .map_err(|error| error.to_string())?;

    let response: Value = client
        .post(&url)
        .json(&json!({
            "model": model.trim(),
            "messages": [
                { "role": "system", "content": system_prompt },
                message
            ],
            "format": "json",
            "stream": false,
            "options": { "temperature": 0.2 }
        }))
        .send()
        .map_err(|error| {
            if error.is_connect() {
                format!("Cannot reach local Ollama at {url}. Start Ollama and check the server address.")
            } else {
                error.to_string()
            }
        })?
        .error_for_status()
        .map_err(|error| format!("Local Ollama returned an HTTP error: {error}"))?
        .json()
        .map_err(|error| format!("Could not parse local Ollama response: {error}"))?;

    let content = response
        .pointer("/message/content")
        .and_then(Value::as_str)
        .ok_or_else(|| "The local model returned no coach response.".to_owned())?;

    let parsed: Value = serde_json::from_str(content).unwrap_or_else(|_| {
        json!({
            "current_goal": "",
            "scene_summary": "",
            "next_step": content,
            "question": "",
            "confidence": "low",
            "learning": ""
        })
    });

    let get = |key: &str| {
        parsed
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .to_owned()
    };

    let mut update = GameCoachUpdate {
        game_name: game_name.trim().to_owned(),
        current_goal: get("current_goal"),
        scene_summary: get("scene_summary"),
        next_step: get("next_step"),
        question: get("question"),
        confidence: get("confidence"),
        learning: get("learning"),
    };

    if update.next_step.is_empty() && update.question.is_empty() {
        update.next_step = "I couldn't confidently identify the next step. Tell me what you are trying to do in this game.".into();
        update.confidence = "low".into();
    }
    if !matches!(update.confidence.as_str(), "low" | "medium" | "high") {
        update.confidence = "low".into();
    }

    Ok(update)
}

/// Check whether the local Ollama server responds to its tags endpoint.
pub fn check_local_server(endpoint: &str) -> Result<Vec<String>, String> {
    let url = format!("{}/api/tags", endpoint.trim().trim_end_matches('/'));
    let response: Value = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(8))
        .build()
        .map_err(|error| error.to_string())?
        .get(&url)
        .send()
        .map_err(|error| format!("Local AI server is unreachable: {error}"))?
        .error_for_status()
        .map_err(|error| format!("Local AI server returned an HTTP error: {error}"))?
        .json()
        .map_err(|error| format!("Could not read local AI server response: {error}"))?;

    let models = response
        .get("models")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.get("name").and_then(Value::as_str))
                .map(str::to_owned)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    Ok(models)
}
