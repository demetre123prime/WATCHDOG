# Contributing

Read [AGENTS.md](AGENTS.md) and [docs/PRIVACY_AND_SECURITY.md](docs/PRIVACY_AND_SECURITY.md) before changing behavior.

## Suggested areas
- Improve capture reliability and make the Watch state unmistakable.
- Improve the native interface and accessibility.
- Add support for local vision models through Ollama.
- Improve user-controlled text notes and app/game interface profiles.
- Add interface discovery without collecting private account data.
- Explore guided UI actions only when they require a separate user approval for every action.

## Before a pull request
- Run `cargo fmt --all`.
- Run `cargo check --workspace --all-targets`.
- Describe the change and include test results.
- Do not include screenshots, private logs, credentials, cookies, or tokens.

Changes that introduce stealth monitoring, credential theft, or unapproved control of other apps are out of scope.
