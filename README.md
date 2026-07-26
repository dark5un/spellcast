# Spellcast — Dictation-First Terminal Keyboard Multiplexer

Spellcast is a dictation-first terminal keyboard multiplexer for Linux that lets you speak your commands, code, and prose instead of typing them. It provides token-aware speech-to-text with inline editing, phonetic prediction, and a concept-to-word "explain" feature.

This repository contains a **terminal PTY wrapper** — a working spike that intercepts input at the terminal level for development, testing, and standalone use. The dictation pipeline (audio capture → ASR → tokenization → injection) is the core; the output sink is swappable. A future **uinput injector** spike will drive `/dev/uinput` directly, replacing standard input system-wide as the core input component of [Chaossynergy](https://github.com/Chaossynergy) — an agent-native immutable Linux OS.

## Features

- **Two modes**: Dictation (Caps Lock ON) and Raw passthrough (Caps Lock OFF)
- **Token navigation**: Navigate between tokens (not words) with H/L keys
- **Phonetic predictions**: Up to 3 alternatives ranked by phoneme distance
- **Explain feature**: Describe a concept verbally, get the right token
- **Kill switch**: Ctrl+Shift+Escape immediately disables Spellcast
- **Local only**: All processing runs on your machine — no cloud
- **GPU acceleration**: CUDA (NVIDIA) and CPU backends with auto-detection
- **Persistent memory**: Learns from your corrections over time via SQLite
- **Packaging options**: RPM (Fedora), Flatpak, and AUR (Arch Linux) packages with an AppStream metadata and a model management CLI (`spellcast models download/list/update`)
- **Plugin system**: Extensible via the `SpellcastPlugin` trait — load, register, unload, and list plugins at runtime through `spellcast plugins` CLI commands; includes built-in `MedicalDictionaryPlugin` and `CodeSymbolsPlugin`
- **Accessibility**: Audio feedback (beep tones on mode transitions), screen reader events via `spd-say`, and a 7-step onboarding wizard for first-run setup

## Quick Start

### Prerequisites

- Linux (Bazzite/Fedora Silverblue primary target)
- `podman` and `distrobox` (pre-installed on Bazzite)
- Microphone (built-in or external)

### Setup

Run the setup script to create a distrobox container with all dependencies:

```bash
git clone https://github.com/dark5un/spellcast.git
cd spellcast

# For NVIDIA GPU systems:
./scripts/setup-bazzite.sh --nvidia

# For CPU-only systems:
./scripts/setup-bazzite.sh

# Enter the development environment:
distrobox enter spellcast-dev
```

### Build

```bash
# Inside the distrobox container
cargo build --release

# Download the ASR model:
./scripts/download-models.sh --asr-only
```

### Run

```bash
cargo run --release
```

## Documentation

| Document | Description |
|---|---|
| [ARCHITECTURE.md](docs/ARCHITECTURE.md) | System architecture and data flow |
| [BAZZITE.md](docs/BAZZITE.md) | Bazzite Linux setup guide |
| [CONFIGURATION.md](docs/CONFIGURATION.md) | Full configuration reference |
| [DEVELOPMENT.md](docs/DEVELOPMENT.md) | Development setup and TDD workflow |
| [PLAN.md](PLAN.md) | Implementation plan |
| [RESEARCH.md](RESEARCH.md) | Research findings and crate evaluations |

## Configuration

Spellcast is configured via `~/.config/spellcast/config.toml`. A default config is generated on first run.

## License

Apache License 2.0