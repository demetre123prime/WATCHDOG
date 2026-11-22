# Development

## Requirements
- Rust stable toolchain (Rust 1.88 or newer recommended).
- Windows 10/11 for the primary target.
- Ollama running locally for visual question answering.
- A vision-capable model, such as `qwen2.5vl:3b`.

## Build
```powershell
ollama pull qwen2.5vl:3b
cargo fmt --all
cargo check --workspace --all-targets
cargo run -p watchdog-desktop
cargo build --release -p watchdog-desktop
```

The Windows executable is generated at `target\release\watchdog.exe`.

## Project structure
The workspace separates UI, core state, capture, local AI, and optional text memory so contributors can work on one subsystem at a time.

## Troubleshooting
- If the UI opens but visual answers fail, check that Ollama is running at `http://127.0.0.1:11434`.
- Run `ollama list` and ensure the model field matches a downloaded model.
- The app will not capture anything until Watch is visibly enabled.
