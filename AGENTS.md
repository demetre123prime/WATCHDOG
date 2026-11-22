# Instructions for AI coding agents

Read this file before changing WATCHDOG.

## Product contract
- Preserve the original WATCHDOG idea: a visual assistant that learns user interfaces with the user's knowledge and control.
- The app must start with WATCH disabled.
- Only capture when the visible WATCH control is enabled.
- STOP must close the capture gate and clear the last in-memory frame.
- Never add hidden monitoring, keylogging, credential/cookie/token collection, silent startup, or stealth persistence.
- Keep screen images on the device. The current vision client posts only to the local Ollama endpoint.
- Do not add automatic clicks or typing. Any future action must be described and approved by the user for that individual action.
- Persistent memory must be user-approved, inspectable, editable, and deletable. Do not store screenshots in the memory database.

## Workspace
- `apps/watchdog-desktop`: native eframe/egui app.
- `crates/watchdog-core`: capture gate and frame model.
- `crates/watchdog-capture`: XCap capture worker.
- `crates/watchdog-ai`: local Ollama vision/chat client.
- `crates/watchdog-memory`: SQLite text observations.

## Quality checks
Run:
```powershell
cargo fmt --all
cargo check --workspace --all-targets
cargo build --release -p watchdog-desktop
```

Keep the Windows GitHub Actions workflow green. Explain any behavior change and how it was tested.
