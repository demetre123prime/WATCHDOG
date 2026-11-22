# Roadmap

## Milestone 0 — foundations
- [x] Rust workspace and native desktop shell.
- [x] Visible Watch toggle, OFF at launch.
- [x] In-memory screen preview with Stop control.
- [x] Local Ollama vision requests.
- [x] User-approved SQLite text observations.
- [x] Windows x64 build workflow.

## Milestone 0.5 — guided game learning
- [x] User-started game session with Roblox as the default profile name.
- [x] Always-on-top top-left game guide HUD.
- [x] Periodic local vision analysis with one suggested next step.
- [x] Ask one player question when a game mechanic is unclear; wait for the answer.
- [x] Reuse player explanations and explicitly saved game notes in later analysis.
- [x] Local Ollama server connectivity check and configurable address.

## Milestone 1 — interface understanding
- [ ] OCR and text/element extraction.
- [ ] Detect the active app or game without collecting account data.
- [ ] Searchable app/game profiles.
- [ ] User-approved descriptions of menus, controls, and recurring interface elements.
- [ ] Better capture selection (monitor/window) with clear user choice.

## Milestone 2 — guided discovery
- [ ] Natural-language interface questions such as “where is the shop?”
- [ ] Explain visible controls and common UI flows.
- [ ] Let the user correct and update interface notes.
- [ ] Inspect, edit, export, and delete learned notes.

## Milestone 3 — user-approved assistance
- [ ] Optional guided actions, initially one step at a time.
- [ ] Explain the exact proposed action before it happens.
- [ ] Require a fresh explicit approval for each click or keystroke.
- [ ] Never run unattended macros or hidden automation.

## Non-goals
- Hidden surveillance or remote access.
- Credential, cookie, token, or keystroke collection.
- Uploading screenshots to a cloud model by default.
- Autonomous control of games or other applications.
