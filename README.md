# Spellcast — Dictation-First Terminal Keyboard Multiplexer

Spellcast is a dictation-first terminal keyboard multiplexer for Linux that lets you speak your commands, code, and prose instead of typing them. It provides token-aware speech-to-text with inline editing, phonetic prediction, and a concept-to-word "explain" feature.

This repository contains a **terminal PTY wrapper** — a working spike that intercepts input at the terminal level for development, testing, and standalone use. The dictation pipeline (audio capture → VAD → ASR → tokenization → injection) is the core; the output sink is swappable. A future **uinput injector** spike will drive `/dev/uinput` directly, replacing standard input system-wide as the core input component of [Chaossynergy](https://github.com/Chaossynergy) — an agent-native immutable Linux OS.

## Features

- **Two modes**: Dictation and Raw passthrough, toggled with Ctrl+Space (Caps Lock also works if the terminal supports the kitty keyboard protocol)
- **Continuous dictation**: VAD-based listening — just speak, no key press needed to start/stop recording
- **Token navigation**: Navigate between tokens (not words) with `h`/`l` keys
- **Token editing**: `x` deletes the highlighted token, `r` re-dictates it
- **Phonetic predictions**: Up to 3 alternatives ranked by phoneme distance; accept with `1`/`2`/`3`
- **Explain feature**: `e` on a token triggers concept-to-word lookup (DB cache → LLM → web search). *Note: DB lookup works, but the LLM path is not yet wired into the event loop.*
- **Kill switch**: Ctrl+G immediately disables Spellcast; Ctrl+G again re-enables it
- **Local only**: All processing runs on your machine — no cloud
- **GPU acceleration**: CUDA (NVIDIA, default), Vulkan, and CPU backends. Backend choice is a runtime config/CLI option, not a compile-time choice
- **Persistent memory**: Learns from your corrections over time via SQLite
- **Plugin system**: Extensible via the `SpellcastPlugin` trait — built-in `MedicalDictionaryPlugin` and `CodeSymbolsPlugin`
- **Accessibility**: Audio feedback (beep tones on mode transitions), screen reader events via `spd-say`, and an 8-step onboarding wizard for first-run setup

## Quick Start

### Prerequisites

- Linux (Bazzite/Fedora Silverblue primary target)
- `podman` and `distrobox` (pre-installed on Bazzite)
- Microphone (built-in or external)
- PipeWire audio system (cpal uses the PipeWire backend)

### Setup

Run the setup script to create a distrobox container with all dependencies:

```bash
git clone https://github.com/dark5un/spellcastv1.git
cd spellcastv1

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
# CUDA is the default feature. For CPU-only or Vulkan:
#   cargo build --release --no-default-features --features cpu
#   cargo build --release --no-default-features --features vulkan
cargo build --release

# Download the ASR model:
./scripts/download-models.sh --asr-only
```

### Run

```bash
# List available audio input devices:
cargo run --release -- --check-audio

# Set a specific input device (saved to config):
cargo run --release -- --set-input-device "device-name"

# Start Spellcast (verbose logging to ~/.config/spellcast/spellcast.log):
cargo run --release -- -v

# Specify a compute backend at runtime:
cargo run --release -- --backend cpu
cargo run --release -- --backend cuda
```

### CLI Options

| Option | Description |
|--------|-------------|
| `-c, --config <PATH>` | Path to config file (default: `~/.config/spellcast/config.toml`) |
| `-b, --backend <TYPE>` | Compute backend override: `auto`, `cuda`, `vulkan`, `cpu` |
| `-s, --shell <PATH>` | Shell to spawn (default: `$SHELL` or `/bin/bash`) |
| `-v, --verbose` | Enable verbose (debug) logging to log file |
| `--check-audio` | List numbered, deduplicated input devices and exit |
| `--set-input-device <NAME>` | Save an input device name to config and exit |

## Key Bindings

| Key | Mode | Action |
|-----|------|--------|
| Ctrl+Space | Any | Toggle between Dictation and Raw mode |
| Caps Lock | Any | Toggle mode (requires kitty keyboard protocol) |
| Ctrl+G | Any | Toggle kill switch (disable/re-enable Spellcast) |
| `h` / Left | Dictation | Navigate to previous token |
| `l` / Right | Dictation | Navigate to next token |
| `x` | Dictation | Delete highlighted token |
| `r` | Dictation | Re-dictate highlighted token (deletes it, starts new recording) |
| `e` | Dictation | Explain: trigger concept-to-word lookup on highlighted token |
| `1`/`2`/`3` | Dictation | Accept prediction 1, 2, or 3 |
| Space | Dictation | Push-to-talk: start a 3-second recording |
| Enter | Dictation | Send Enter to PTY |
| Backspace | Dictation | Send Backspace to PTY |
| Esc | Dictation | Clear token selection |
| Any other key | Dictation | Pass through to PTY |
| Any key | Raw | Pass through to PTY (Spellcast transparent) |

## Documentation

| Document | Description |
|---|---|
| [ARCHITECTURE.md](docs/ARCHITECTURE.md) | System architecture and data flow |
| [BAZZITE.md](docs/BAZZITE.md) | Bazzite Linux setup guide |
| [PLAN.md](PLAN.md) | Implementation plan |
| [RESEARCH.md](RESEARCH.md) | Research findings and crate evaluations |

## Configuration

Spellcast is configured via `~/.config/spellcast/config.toml`. A default config is generated on first run if the file is missing. The default config template is at `config/default-config.toml`.

Logs are written to `~/.config/spellcast/spellcast.log` (not stderr) to avoid corrupting the terminal display.

## Feature Model

All core functionality (tree-sitter, VAD, LLM, ASR) is always compiled — there are no optional feature flags for these. The only compile-time features are GPU backend SDK selection:

| Feature | Description |
|---------|-------------|
| `cuda` (default) | Build with CUDA toolkit support (NVIDIA) |
| `vulkan` | Build with Vulkan SDK support |
| `cpu` | CPU-only build (no GPU SDK required) |

The runtime choice of which backend to use (`auto`/`cuda`/`vulkan`/`cpu`) is made via config or the `--backend` CLI flag, regardless of which features were compiled in.

## License

Apache License 2.0
