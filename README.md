# WATCHDOG

**A local-first Rust visual AI assistant that understands the screen when you explicitly turn WATCH on.**

WATCHDOG is built around one idea: you should be able to open an unfamiliar app or game, enable WATCH, and ask a local AI what is on the screen, where a menu is, or how an interface works. Over time, it can build a user-controlled local memory of interfaces it has learned.

> **The original idea is the product requirement.** WATCHDOG is not intended to become a generic autonomous computer-control bot. Screen observation is opt-in, visible, and off by default. Future actions must be individually approved by the user.

## What this first milestone includes

- Native Windows desktop shell written in Rust with egui/eframe.
- An explicit WATCH ON/OFF control. Capture starts only after the user turns it on.
- A live preview of the primary monitor while WATCH is on.
- Periodic screen snapshots through [XCap](https://github.com/nashaofu/xcap).
- Local Ollama chat. When WATCH is on, the current screenshot is attached to the local vision-model request. When WATCH is off, no screenshot is sent.
- A local SQLite memory store for observations that the user explicitly chooses to save.
- Modular crates so different contributors and coding agents can work on independent parts.

## Game guide (first implementation)

- Start a visible **game-learning session** before or after joining a Roblox game. Roblox is the default, but the game name can be changed.
- WATCHDOG opens a small always-on-top guide HUD near the top-left of the desktop.
- The local vision model inspects the current screen during the session and suggests one next step, such as opening a shop or trying a visible in-game control.
- When it cannot understand a mechanic, it asks one short question and waits for the player's explanation before continuing.
- Your explanation is included in later local AI requests. Use **Save lesson** to deliberately store a useful text finding in local SQLite memory.
- Automatic guidance can be switched off, and **Stop guide** turns WATCH off and clears the latest frame.

This is a live, periodic screen-observation session, **not a video recorder**. Frames are held temporarily in memory and are not saved as video files. The floating HUD works best with Roblox in windowed or borderless mode; some exclusive-fullscreen games may cover other windows.

## What is not implemented yet

OCR extraction, app/game detection, automatic interface learning, semantic search, persistent chat history, window-specific capture, model downloads, and approved UI actions are roadmap work. The app does not secretly click, type, navigate, or control other applications.

## Quick start (Windows)

Requirements:
- Rust stable toolchain (Rust 1.88+ recommended)
- [Ollama](https://ollama.com/) running locally
- A vision-capable Ollama model

Install a small vision model:

```powershell
ollama pull qwen2.5vl:3b
```

Build and run:

```powershell
git clone https://github.com/demetre123prime/WATCHDOG.git
cd WATCHDOG
cargo run -p watchdog-desktop --release
```

In WATCHDOG, set the model field to `qwen2.5vl:3b`, click **Start WATCH**, and ask a question about the visible screen. Ollama is accessed only through `http://127.0.0.1:11434`; images are not uploaded to a hosted service by this application.

## Workspace map

```text
apps/
  watchdog-desktop/     Native Rust desktop UI
crates/
  watchdog-core/        Watch state and product-level invariants
  watchdog-capture/     Opt-in screen capture worker
  watchdog-ai/          Local Ollama client
  watchdog-memory/      User-controlled SQLite observations
docs/
  ARCHITECTURE.md
  DEVELOPMENT.md
  PRIVACY_AND_SECURITY.md
  ROADMAP.md
  OPEN_SOURCE_REFERENCES.md
AGENTS.md               Shared instructions for AI coding agents
```

## Product principles

1. **WATCH is off by default.** No screen capture before the user explicitly enables it.
2. **Visible observation.** The UI must clearly show when capture is active and provide an immediate stop control.
3. **Local-first.** Screenshots and prompts are sent only to the configured local Ollama endpoint by default.
4. **No silent image retention.** Captured frames are held temporarily in memory; the app does not save them to disk by default.
5. **Memory belongs to the user.** Persisted observations are saved deliberately and can be cleared from the app.
6. **No unapproved actions.** Any future keyboard or mouse action requires a clear explanation and an explicit per-action confirmation.
7. **Modular development.** Preserve the original WATCHDOG purpose when adding features.

## Contributing

Read [AGENTS.md](AGENTS.md), [CONTRIBUTING.md](CONTRIBUTING.md), and [the architecture](docs/ARCHITECTURE.md) before making changes. Keep pull requests focused and run:

```powershell
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

## License

MIT. See [LICENSE](LICENSE).
