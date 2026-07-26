#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
#
# setup-bazzite.sh — Prepare a Bazzite Linux system for Spellcast development.
#
# This script:
# 1. Creates a distrobox container using fedora:latest as the base image
# 2. Installs build dependencies inside the container
# 3. Sets up the udev rule for /dev/uinput on the host
# 4. Verifies audio, uinput, and GPU access from inside the container
#
# Prerequisites:
#   - Bazzite Linux (or any Fedora Silverblue/Kinoite derivative)
#   - podman (pre-installed on Bazzite)
#   - distrobox (pre-installed on Bazzite)
#   - NVIDIA drivers installed on host (if using NVIDIA GPU)
#
# Usage:
#   ./setup-bazzite.sh [--name spellcast-dev] [--nvidia]
#
# Options:
#   --name NAME    Container name (default: spellcast-dev)
#   --nvidia       Include NVIDIA GPU passthrough (--nvidia flag for distrobox)
#   --help         Show this help

set -euo pipefail

CONTAINER_NAME="spellcast-dev"
USE_NVIDIA=false

# Parse arguments
while [[ $# -gt 0 ]]; do
    case "$1" in
        --name)
            CONTAINER_NAME="$2"
            shift 2
            ;;
        --nvidia)
            USE_NVIDIA=true
            shift
            ;;
        --help)
            echo "Usage: $0 [--name spellcast-dev] [--nvidia]"
            exit 0
            ;;
        *)
            echo "Unknown option: $1"
            echo "Usage: $0 [--name spellcast-dev] [--nvidia]"
            exit 1
            ;;
    esac
done

echo "=== Spellcast Bazzite Setup ==="
echo "Container name: ${CONTAINER_NAME}"
echo "NVIDIA support: ${USE_NVIDIA}"
echo ""

# ---------------------------------------------------------------
# Step 1: Set up udev rule for /dev/uinput on the host
# ---------------------------------------------------------------
echo "=== Step 1: Setting up /dev/uinput udev rule ==="

UINPUT_RULE_FILE="/etc/udev/rules.d/99-spellcast-uinput.rules"
if [ ! -f "${UINPUT_RULE_FILE}" ]; then
    echo 'KERNEL=="uinput", SUBSYSTEM=="misc", MODE="0660", GROUP="input"' | sudo tee "${UINPUT_RULE_FILE}"
    sudo udevadm control --reload-rules
    sudo udevadm trigger
    echo "udev rule created and triggered."
    echo ""
    echo "  This rule is the foundation for both the PTY-wrapper mode and"
    echo "  the future system-wide uinput injector for Chaossynergy."
    echo "  It won't need to change when the integration surface evolves."
else
    echo "udev rule already exists at ${UINPUT_RULE_FILE}"
fi

# Ensure user is in the input group
if ! groups "${USER}" | grep -q '\binput\b'; then
    echo "Adding user ${USER} to the 'input' group..."
    sudo usermod -aG input "${USER}"
    echo "NOTE: You may need to log out and back in for group changes to take effect."
else
    echo "User ${USER} is already in the 'input' group."
fi

echo ""

# ---------------------------------------------------------------
# Step 2: Create the distrobox container
# ---------------------------------------------------------------
echo "=== Step 2: Creating distrobox container ==="

# Check if container already exists
if distrobox list 2>/dev/null | grep -q "${CONTAINER_NAME}"; then
    echo "Container '${CONTAINER_NAME}' already exists."
    echo "To re-create, run: distrobox rm --force ${CONTAINER_NAME}"
    echo ""
    read -rp "Enter the container anyway and continue setup? [Y/n] " -n 1 REPLY
    echo ""
    REPLY=${REPLY:-Y}
    if [[ ! "$REPLY" =~ ^[Yy]$ ]]; then
        exit 0
    fi
else
    echo "Creating container '${CONTAINER_NAME}' from fedora:latest..."

    NVIDIA_FLAG=""
    if [ "${USE_NVIDIA}" = true ]; then
        NVIDIA_FLAG="--nvidia"
        echo "NVIDIA passthrough enabled."
    fi

    # Device flags for uinput and NVIDIA GPU
    DEVICE_FLAGS="--device /dev/uinput"
    if [ "${USE_NVIDIA}" = true ]; then
        DEVICE_FLAGS="${DEVICE_FLAGS} --device /dev/nvidia0 --device /dev/nvidiactl --device /dev/nvidia-uvm"
    fi

    distrobox create \
        --name "${CONTAINER_NAME}" \
        --image fedora:latest \
        ${NVIDIA_FLAG} \
        --additional-flags "${DEVICE_FLAGS}" \
        --additional-packages "sudo git curl wget"

    echo "Container created successfully."
fi

echo ""

# ---------------------------------------------------------------
# Step 3: Install build dependencies inside the container
# ---------------------------------------------------------------
echo "=== Step 3: Installing build dependencies inside container ==="

distrobox enter "${CONTAINER_NAME}" -- bash -c '
    set -euo pipefail

    echo "Updating packages..."
    sudo dnf upgrade -y

    echo "Installing Rust toolchain..."
    if ! command -v rustc &>/dev/null; then
        curl --proto =https --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
        source "$HOME/.cargo/env"
    fi
    rustup update

    echo "Installing C/C++ toolchain and build tools..."
    sudo dnf install -y \
        gcc \
        gcc-c++ \
        make \
        cmake \
        pkg-config \
        findutils

    echo "Installing audio libraries..."
    sudo dnf install -y \
        alsa-lib-devel \
        pipewire-devel \
        pipewire-alsa \
        pulseaudio-libs-devel

    echo "Installing SQLite development headers..."
    sudo dnf install -y \
        libsqlite3x-devel

    echo "Installing udev development headers..."
    sudo dnf install -y \
        systemd-devel

    echo "Installing CUDA toolkit (for NVIDIA GPU support)..."
    # For fedora:latest, CUDA is available via RPM Fusion or NVIDIA repo
    # Try RPM Fusion first
    sudo dnf install -y \
        dnf-plugins-core 2>/dev/null || true

    # Install CUDA if NVIDIA is available
    if command -v nvidia-smi &>/dev/null; then
        echo "NVIDIA GPU detected — ensuring CUDA compatibility..."
        # On fedora:latest inside distrobox, CUDA may be available via
        # the host NVIDIA library bindings (from --nvidia flag).
        # For native CUDA development, install the CUDA toolkit:
        sudo dnf install -y cuda-toolkit 2>/dev/null || \
        echo "NOTE: cuda-toolkit not in default repos. CUDA support will use host libraries."
        echo "CUDA check:"
        nvidia-smi 2>/dev/null || echo "nvidia-smi not available (expected if no --nvidia flag)"
    else
        echo "No NVIDIA GPU detected in container."
    fi

    echo "Verifying Rust installation..."
    rustc --version
    cargo --version

    echo ""
    echo "=== Build dependencies installed ==="
'

echo ""

# ---------------------------------------------------------------
# Step 4: Verify device/audio/GPU access
# ---------------------------------------------------------------
echo "=== Step 4: Verifying device, audio, and GPU access ==="

echo ""
echo "--- /dev/uinput access ---"
distrobox enter "${CONTAINER_NAME}" -- bash -c '
    if [ -c /dev/uinput ]; then
        echo "  ✅ /dev/uinput is accessible (character device)"
        ls -la /dev/uinput
    else
        echo "  ❌ /dev/uinput not found or not a character device"
        echo "  Check udev rule and ensure user is in the input group."
    fi
'

echo ""
echo "--- Audio (PipeWire) access ---"
distrobox enter "${CONTAINER_NAME}" -- bash -c '
    # Check if PipeWire socket is accessible
    PW_SOCKET="/run/user/$(id -u)/pipewire-0"
    if [ -S "${PW_SOCKET}" ] || [ -e "${PW_SOCKET}" ]; then
        echo "  ✅ PipeWire socket found at ${PW_SOCKET}"
    else
        echo "  ❌ PipeWire socket not found at ${PW_SOCKET}"
        echo "  Check that distrobox shares the PipeWire socket."
        echo "  Try: ls -la /run/user/$(id -u)/ | grep pipewire"
    fi

    # Check if pw-record is available
    if command -v pw-record &>/dev/null; then
        echo "  ✅ pw-record available for audio recording"
    else
        echo "  ℹ️  pw-record not available (install pipewire-utils for it)"
    fi
'

echo ""
echo "--- GPU access ---"
distrobox enter "${CONTAINER_NAME}" -- bash -c '
    # Check NVIDIA GPU
    if command -v nvidia-smi &>/dev/null; then
        echo "  ✅ nvidia-smi available"
        nvidia-smi --query-gpu=name,driver_version --format=csv,noheader 2>/dev/null || \
        echo "  ⚠️  nvidia-smi failed but command exists"
    else
        echo "  ℹ️  No NVIDIA GPU detected in container."
        echo "  If you have an NVIDIA GPU, re-create with: distrobox create --nvidia"
    fi

    # Check Vulkan
    if command -v vulkaninfo &>/dev/null; then
        echo "  ✅ vulkaninfo available"
        vulkaninfo --summary 2>/dev/null | head -5 || true
    else
        echo "  ℹ️  vulkaninfo not available (install vulkan-tools for it)"
    fi
'

echo ""
echo "--- Rust toolchain ---"
distrobox enter "${CONTAINER_NAME}" -- bash -c '
    rustc --version
    cargo --version
'

echo ""
echo ""
echo "=== Setup Complete ==="
echo ""
echo "To enter the development environment:"
echo "  distrobox enter ${CONTAINER_NAME}"
echo ""
echo "To build Spellcast:"
echo "  cd /path/to/spellcast"
echo "  cargo build"
echo ""
echo "To run Spellcast (from inside the container):"
echo "  cargo run -- --help"
echo ""
echo "NOTE: If /dev/uinput access fails:"
echo "  1. Reboot (or log out and back in) to apply group changes"
echo "  2. Run: sudo udevadm control --reload-rules && sudo udevadm trigger"
echo "  3. Verify: ls -la /dev/uinput"
echo ""