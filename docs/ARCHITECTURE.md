# Architecture

## Workspace crates
- `watchdog-core`: OFF-by-default WatchGate and temporary CapturedFrame data model.
- `watchdog-capture`: XCap worker that polls the gate and keeps the latest image in RAM.
- `watchdog-ai`: PNG-in-memory encoding and local Ollama chat requests.
- `watchdog-memory`: SQLite text observations explicitly saved by the user.
- `watchdog-desktop`: eframe/egui native UI, preview, chat, and memory controls.

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
