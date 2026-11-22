# Privacy and security

WATCHDOG is built around visible, user-controlled observation.

## Current implementation
- WATCH starts OFF.
- The capture worker checks the explicit WatchGate before calling the screen-capture API.
- STOP closes the gate and clears the last in-memory frame.
- Frames are held temporarily in RAM; the app does not save screenshot files.
- Vision questions use the Ollama API on `127.0.0.1:11434`.
- SQLite stores only text notes the user explicitly saves.
- There is no analytics, telemetry, account sign-in, cloud image upload, keylogging, credential collection, or automatic clicking.

## Game guide sessions
- A game guide session begins only when the player presses **Start game guide**.
- During a session, periodic still frames are sent to the configured local Ollama server. There is no video recording or screenshot-file retention.
- The top-left guide window is a visible, always-on-top overlay and can be hidden or stopped.
- The model only recommends actions and asks questions; it does not click, type, or control the game.
- Player explanations remain in session memory unless the player explicitly saves a lesson to SQLite.

## User precautions
Do not enable Watch while passwords, private messages, payment details, or other sensitive content are visible. Any local vision model that receives a frame can analyze everything visible in that frame.

## Memory
The SQLite database is stored in the user's local application-data directory under `WATCHDOG/memory.sqlite3`. Users can inspect saved notes in the app and clear all notes from its memory panel. Screenshots are not stored in this database.
