# Spellcast — Bazzite Setup Guide

> Target platform: **Bazzite Linux** (Fedora Silverblue/Kinoite derivative)
> Tested on: Bazzite 42 / Fedora 42 Atomic

Spellcast targets Bazzite because it provides a modern, immutable Linux desktop with Podman and Distrobox pre-installed — a perfect foundation for the dictation-first terminal multiplexer's containerized development and runtime model.

---

## Table of Contents

- [Overview](#overview)
- [Prerequisites](#prerequisites)
- [Step 1: Uinput Device Access (Host)](#step-1-uinput-device-access-host)
- [Step 2: Create the Distrobox Container](#step-2-create-the-distrobox-container)
- [Step 3: Install Build Dependencies](#step-3-install-build-dependencies)
- [Step 4: Download Models](#step-4-download-models)
- [Step 5: Build Spellcast](#step-5-build-spellcast)
- [Step 6: Verify Everything Works](#step-6-verify-everything-works)
- [NVIDIA GPU Setup](#nvidia-gpu-setup)
- [Audio Setup Details](#audio-setup-details)
- [Troubleshooting](#troubleshooting)
- [Quick Reference](#quick-reference)

---

## Overview

Bazzite is an immutable (atomic) Fedora variant optimized for gaming and development. Unlike traditional Linux distributions, you **do not** install packages directly on the host. Instead:

| Concept | Bazzite Approach |
|---|---|
| Package management | `rpm-ostree` (layered packages discouraged) |
| Development environment | **Distrobox** containers (recommended) |
| System configuration | `ujust` commands + `rpm-ostree` overrides |
| Device passthrough | udev rules + container device bind mounts |

Spellcast follows this model: all build tools and runtimes live inside a Distrobox container. The only host-level changes are:
1. A udev rule for `/dev/uinput` (keyboard injection)
2. NVIDIA GPU drivers if using a dedicated GPU (standard Bazzite install)

---

## Prerequisites

### On Bazzite (pre-installed)

- **Podman** — container runtime (`podman --version`)
- **Distrobox** — container manager (`distrobox --version`)
- **Microphone** — built-in or external (test with `pw-record --list-targets`)

### On the Host (general Linux)

- **Linux** (Fedora Silverblue, Fedora Workstation, or any recent distro)
- **Podman** or Docker installed
- **Distrobox** installed
- **Microphone** accessible via PipeWire or ALSA

```bash
# Verify prerequisites
podman --version
distrobox --version
pw-record --list-targets 2>/dev/null || echo "PipeWire not found (install pipewire-utils)"
```

---

## Step 1: Uinput Device Access (Host)

Spellcast injects keystrokes into the terminal via the `/dev/uinput` kernel device. This requires a udev rule and group membership on the **host** (Bazzite).

> This udev rule is the foundation for both the current PTY-wrapper mode and the future system-wide uinput injector that will become the core input component of [Chaossynergy](https://github.com/dark5un/Chaossynergy). The same rule works for both. The host setup won't need to change when the uinput spike replaces the PTY path.

### 1a. Create the udev rule

```bash
echo 'KERNEL=="uinput", SUBSYSTEM=="misc", MODE="0660", GROUP="input"' | \
  sudo tee /etc/udev/rules.d/99-spellcast-uinput.rules

sudo udevadm control --reload-rules
sudo udevadm trigger
```

### 1b. Add yourself to the `input` group

```bash
sudo usermod -aG input $USER
```

> **Important**: Group changes take effect on next login. Either log out and back in, or start a new login shell with `su - $USER` to apply them without rebooting.

### 1c. Verify

```bash
ls -la /dev/uinput
# Should show: crw-rw---- (character device, group "input")
groups | grep input
# Should include "input" in the output
```

---

## Step 2: Create the Distrobox Container

Spellcast runs inside a `fedora:latest` Distrobox container with device access.

### Basic container (CPU-only)

```bash
distrobox create \
    --name spellcast-dev \
    --image fedora:latest \
    --additional-flags "--device /dev/uinput" \
    --additional-packages "sudo git curl wget"
```

### NVIDIA container (with GPU passthrough)

```bash
distrobox create \
    --name spellcast-dev \
    --image fedora:latest \
    --nvidia \
    --additional-flags "--device /dev/uinput --device /dev/nvidia0 --device /dev/nvidiactl --device /dev/nvidia-uvm" \
    --additional-packages "sudo git curl wget"
```

> **Note**: The `--nvidia` flag bind-mounts the host's NVIDIA libraries into the container. The `--device` flags provide direct GPU device access for CUDA compute.

---

## Step 3: Install Build Dependencies

Enter the container and install everything needed to build Spellcast.

```bash
distrobox enter spellcast-dev
```

Inside the container:

```bash
# Update packages
sudo dnf upgrade -y

# Rust toolchain
curl --proto =https --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
source "$HOME/.cargo/env"
rustup update

# C/C++ toolchain and build tools
sudo dnf install -y \
    gcc gcc-c++ make cmake pkg-config findutils

# Audio libraries (ALSA + PipeWire)
sudo dnf install -y \
    alsa-lib-devel \
    pipewire-devel \
    pipewire-alsa \
    pulseaudio-libs-devel

# SQLite development headers
sudo dnf install -y \
    libsqlite3x-devel

# udev development headers (for portable-pty)
sudo dnf install -y \
    systemd-devel

# Optional: NVIDIA CUDA toolkit
sudo dnf install -y dnf-plugins-core
sudo dnf install -y cuda-toolkit 2>/dev/null || \
    echo "CUDA toolkit not in default repos — using host NVIDIA libs"
```

Verify:

```bash
rustc --version
cargo --version
gcc --version
```

> The setup script [`scripts/setup-bazzite.sh`](../scripts/setup-bazzite.sh) automates all of the above. Run it from the host:
> ```bash
> # CPU-only
> ./scripts/setup-bazzite.sh
>
> # With NVIDIA GPU
> ./scripts/setup-bazzite.sh --nvidia
> ```

---

## Step 4: Download Models

Spellcast needs a Whisper ASR model for speech-to-text. From inside the container (or from the host, targeting the container's home):

```bash
# From inside the container
cd /path/to/spellcast
./scripts/download-models.sh --asr-only
```

This downloads the Whisper `base.en` model (~150 MB) from HuggingFace to `~/.config/spellcast/models/ggml-base.en.bin`.

Optional — LLM model for the explain feature:

```bash
./scripts/download-models.sh --llm-only
```

This auto-downloads `Qwen3-4B` on first use when running with `--features llm`.

---

## Step 5: Build Spellcast

From inside the container:

```bash
cd /path/to/spellcast
cargo build --release
```

For GPU-accelerated builds (with CUDA features enabled):

```bash
cargo build --release --features cuda
```

The first build may take several minutes as it compiles `whisper-rs` (which builds `whisper.cpp` from source) and all other dependencies.

---

## Step 6: Verify Everything Works

### Device access check

```bash
# Inside the container
ls -la /dev/uinput
# Should show a character device — e.g. crw-rw---- 1 root input ...
```

### Audio recording check

```bash
# Inside the container
pw-record --list-targets 2>/dev/null || \
    echo "Install pipewire-utils for pw-record"

# Quick capture test (3 seconds to /tmp/test.wav)
pw-record --duration 3 /tmp/test.wav 2>/dev/null || \
    echo "pw-record not available — check PipeWire socket mount"
```

### GPU check

```bash
# Inside the container
nvidia-smi 2>/dev/null || echo "No NVIDIA GPU detected"
```

### Build verification

```bash
# Inside the container, from the spellcast directory
cargo build --release
cargo test
cargo clippy
cargo fmt --check
```

---

## NVIDIA GPU Setup

### Prerequisites

- **NVIDIA drivers installed on Bazzite host** — Bazzite ships NVIDIA drivers via `rpm-ostree` (ujust commands). Verify on the host:

```bash
nvidia-smi
# Should show driver version and GPU info
```

### Distrobox with NVIDIA

When creating the container, the `--nvidia` flag:

1. Bind-mounts the host's NVIDIA driver libraries into the container
2. Sets the `NVIDIA_VISIBLE_DEVICES` and `NVIDIA_DRIVER_CAPABILITIES` environment variables
3. Enables GPU compute access for CUDA

### Inside the container

CUDA-aware crates (`whisper-rs` with `cuda` feature, `mistralrs` with `cuda` feature) will automatically detect and use the GPU.

```bash
# Verify GPU compute from inside container
nvidia-smi
# Or for CUDA version:
ls /usr/lib64/libcuda* 2>/dev/null | head -1
```

### RTX 5090 (Blackwell) Notes

- **Driver**: Bazzite ships NVIDIA driver 575.x+ which supports Blackwell
- **CUDA**: Blackwell needs CUDA 12.8+. If `fedora:latest` doesn't have it, the host's CUDA libraries (from `--nvidia`) provide PTX forward-compatibility — `whisper.cpp` and `mistralrs` ship PTX that JIT-compiles for Blackwell at runtime
- **Fallback**: Spellcast automatically falls back to CPU if GPU is unavailable

---

## Audio Setup Details

### How audio works through Distrobox

Distrobox automatically shares the PipeWire socket from the host into the container at `/run/user/$(id -u)/pipewire-0`. This means:

- **No additional configuration** is needed for most setups
- The container's audio applications communicate with the host's PipeWire daemon
- Spellcast uses `cpal` with its ALSA backend, which talks to PipeWire via `pipewire-alsa`

### Verifying audio inside the container

```bash
# Check PipeWire socket
ls -la /run/user/$(id -u)/pipewire-0
# Should show a socket: srwxr-xr-x ...

# List recording devices
pw-record --list-targets

# Record a 3-second test clip
pw-record --duration 3 /tmp/spellcast-test.wav

# Play it back
pw-play /tmp/spellcast-test.wav
```

### If audio doesn't work

1. Ensure `pipewire-alsa` and `alsa-lib-devel` are installed in the container
2. Check that the PipeWire socket is mounted:
   ```bash
   distrobox enter spellcast-dev -- ls -la /run/user/$(id -u)/ | grep pipewire
   ```
3. On the host, verify microphone access:
   ```bash
   pw-record --list-targets
   ```
4. Restart the container: `distrobox stop spellcast-dev && distrobox enter spellcast-dev`

---

## Troubleshooting

### `/dev/uinput` not accessible from container

| Symptom | Cause | Fix |
|---|---|---|
| `ls -la /dev/uinput` shows `No such file or directory` | udev rule not created | Create the rule (Step 1a), reload, trigger |
| `crw-------` (no group read/write) | Wrong permissions | Fix udev rule to use `MODE="0660", GROUP="input"` |
| `crw-rw----` but still can't open | User not in `input` group | `sudo usermod -aG input $USER`, log out and back in |
| Device visible but container can't access | Container not created with `--device /dev/uinput` | Re-create container with the device flag |

### Audio issues

| Symptom | Cause | Fix |
|---|---|---|
| `pw-record: command not found` | `pipewire-utils` not installed | `sudo dnf install -y pipewire-utils` |
| PipeWire socket missing in container | distrobox not sharing socket | Re-enter container: `distrobox stop && distrobox enter` |
| `pw-record` finds no targets | Microphone not accessible | Check host microphone: `pactl list sources short` |
| ALSA errors in Spellcast | `pipewire-alsa` not installed | `sudo dnf install -y pipewire-alsa alsa-lib-devel` |

### GPU issues

| Symptom | Cause | Fix |
|---|---|---|
| `nvidia-smi: command not found` | `--nvidia` not used at container creation | Re-create with `distrobox create --nvidia` |
| `nvidia-smi` works but CUDA programs fail | CUDA toolkit mismatch | Install CUDA toolkit in container or rely on host libs |
| Build fails on CUDA feature | CUDA paths not configured | `export CUDA_PATH=/usr/lib64/cuda` or use `--features cuda` without CUDA toolkit |

### Build errors

| Error | Likely Fix |
|---|---|
| `whisper-rs` FFI build failure | `export WHISPER_DONT_GENERATE_BINDINGS=1` (use pregenerated bindings) |
| `portable-pty` linking error | Install `systemd-devel` in the container |
| `cpal` ALSA not found | Install `alsa-lib-devel` |
| `libsqlite3x-devel` not found | `sudo dnf install -y libsqlite3x-devel` |

### General

| Issue | Recommendation |
|---|---|
| Container won't start | `distrobox rm --force spellcast-dev && ./scripts/setup-bazzite.sh` |
| Shell stuck in raw mode | `reset` command or open new terminal |
| Permission denied on `/dev/uinput` | Log out and back in, then re-enter container |

---

## Quick Reference

### One-liner setup

```bash
# Clone and enter the repo
git clone https://github.com/spellcast/spellcast.git
cd spellcast

# Run the automated setup script (CPU)
./scripts/setup-bazzite.sh

# Run the automated setup script (NVIDIA GPU)
./scripts/setup-bazzite.sh --nvidia

# Enter the container
distrobox enter spellcast-dev

# Build and run
cargo build --release
cargo run --release
```

### Key paths

| Path | Purpose |
|---|---|
| `~/.config/spellcast/config.toml` | Spellcast configuration file |
| `~/.config/spellcast/models/` | ASR and LLM model storage |
| `~/.local/share/spellcast/` | Persistent memory database |
| `/etc/udev/rules.d/99-spellcast-uinput.rules` | Host udev rule for uinput |

### Useful commands

```bash
# Container management
distrobox enter spellcast-dev         # Enter the container
distrobox stop spellcast-dev          # Stop the container
distrobox rm --force spellcast-dev    # Remove the container (start fresh)

# Container troubleshooting
distrobox list                     # List all containers
podman ps -a                       # List all podman containers

# Audio debugging
pw-record --list-targets           # List audio input devices
pactl list sources short           # List PulseAudio sources
parec --list-devices               # List recording devices

# uinput debugging
ls -la /dev/uinput                 # Check uinput device
udevadm info -a /dev/uinput        # Show udev attributes
udevadm test /sys/class/misc/uinput # Test udev rule application
```

---

## Related Documentation

| Document | Description |
|---|---|
| [ARCHITECTURE.md](ARCHITECTURE.md) | System architecture and data flow |
| [CONFIGURATION.md](CONFIGURATION.md) | Full configuration reference |
| [DEVELOPMENT.md](DEVELOPMENT.md) | Development setup and TDD workflow |
| [README.md](../README.md) | Project overview and quick start |
| [scripts/setup-bazzite.sh](../scripts/setup-bazzite.sh) | Automated setup script |