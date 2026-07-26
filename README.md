# VoxKey — Dictation-First Terminal Keyboard Multiplexer

VoxKey is a dictation-first terminal keyboard multiplexer for Linux that lets you speak your commands, code, and prose instead of typing them. It provides token-aware speech-to-text with inline editing, phonetic prediction, and a concept-to-word "explain" feature.

## Features

- **Two modes**: Dictation (Caps Lock ON) and Raw passthrough (Caps Lock OFF)
- **Token navigation**: Navigate between tokens (not words) with H/L keys
- **Phonetic predictions**: Up to 3 alternatives ranked by phoneme distance
- **Explain feature**: Describe a concept verbally, get the right token
- **Kill switch**: Ctrl+Shift+Escape immediately disables VoxKey
- **Local only**: All processing runs on your machine — no cloud
- **GPU acceleration**: CUDA (NVIDIA) and CPU backends with auto-detection
- **Persistent memory**: Learns from your corrections over time

## Quick Start

### Prerequisites

- Linux (Bazzite/Fedora Silverblue primary target)
- `podman` and `distrobox` (pre-installed on Bazzite)
- Microphone (built-in or external)

### Setup

Run the setup script to create a distrobox container with all dependencies:

```bash
git clone https://github.com/voxkey/voxkey.git
cd voxkey

# For NVIDIA GPU systems:
./scripts/setup-bazzite.sh --nvidia

# For CPU-only systems:
./scripts/setup-bazzite.sh

# Enter the development environment:
distrobox enter voxkey-dev
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

VoxKey is configured via `~/.config/voxkey/config.toml`. A default config is generated on first run.

## License

Apache License 2.0