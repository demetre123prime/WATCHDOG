# Architecture

## Workspace crates
- `watchdog-core`: OFF-by-default WatchGate and temporary CapturedFrame data model.
- `watchdog-capture`: XCap worker that polls the gate and keeps the latest image in RAM.
- `watchdog-ai`: PNG-in-memory encoding and local Ollama chat requests.
- `watchdog-memory`: SQLite text observations explicitly saved by the user.
- `watchdog-desktop`: eframe/egui native UI, preview, chat, and memory controls.

## Game-learning flow
A user explicitly starts a game guide session. This enables the WatchGate and opens a small always-on-top eframe viewport. A background task sends a periodic still frame and approved text context to the configured local Ollama endpoint. The model returns a structured update: current goal, screen summary, one next step, optional question, confidence, and a possible learning note.

If the update contains a question, automatic analysis pauses until the player submits an explanation. The explanation is kept in session memory and used in the next prompt. A lesson is persisted only when the player presses **Save lesson**. The game HUD never performs clicks or keyboard actions.

## Data path
```text
START WATCH (visible)
        |
        v
WatchGate enabled ---> XCap capture ---> latest RGBA frame in RAM
                                               |
Question --------------------------------------+
                                               v
                              Ollama at 127.0.0.1:11434
                                               |
                                               v
                                          UI answer

User-written note ---> explicit Save ---> local SQLite text table
```

With Watch disabled, the capture worker does not call XCap. Pressing STOP disables the gate and clears the latest frame. The SQLite store does not accept image bytes.

## Concurrency
The UI and capture worker share the gate and latest-frame slot. The AI request runs on a separate thread so slow local model responses do not block the UI. The capture worker re-checks the gate while holding the frame lock before publishing a frame.
