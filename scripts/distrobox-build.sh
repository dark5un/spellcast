#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
#
# distrobox-build.sh — Build the Spellcast dev image and create a distrobox container.
#
# This script wraps the Containerfile into the distrobox workflow:
#   1. Builds a local OCI image (spellcast-dev) from Containerfile
#   2. Creates a distrobox container using that image
#   3. Verifies device, audio, and GPU access
#
# Usage:
#   ./scripts/distrobox-build.sh [--name spellcast-dev] [--rebuild]
#
# Flags:
#   --name NAME     Container name (default: spellcast-dev)
#   --rebuild       Force rebuild of the container image
#   --dry-run       Print commands without executing

set -euo pipefail

CONTAINER_NAME="spellcast-dev"
REBUILD=false
DRY_RUN=false
PROJECT_DIR="$(cd "$(dirname "$0")/.." && pwd)"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --name) CONTAINER_NAME="$2"; shift 2 ;;
    --rebuild) REBUILD=true; shift ;;
    --dry-run) DRY_RUN=true; shift ;;
    --help) echo "Usage: $0 [--name spellcast-dev] [--rebuild] [--dry-run]"; exit 0 ;;
    *) echo "Unknown: $1"; exit 1 ;;
  esac
done

run() {
  if [ "$DRY_RUN" = true ]; then echo "  # $*"; else "$@"; fi
}

echo "=== Spellcast Distrobox Builder ==="
echo "  Container: $CONTAINER_NAME"
echo "  Image:     localhost/spellcast-dev:latest"
echo "  Source:    $PROJECT_DIR/Containerfile"
echo ""

# Step 1 — Build the image
echo "--- Building container image ---"
IMAGE_EXISTS=$(podman images --quiet localhost/spellcast-dev 2>/dev/null || true)
if [ -z "$IMAGE_EXISTS" ] || [ "$REBUILD" = true ]; then
  run podman build -t spellcast-dev -f "$PROJECT_DIR/Containerfile" "$PROJECT_DIR"
  echo "  Image built."
else
  echo "  Image exists (use --rebuild to force)."
fi
echo ""

# Step 2 — Create or recreate distrobox
echo "--- Creating distrobox container ---"
EXISTS=$(distrobox list 2>/dev/null | grep -c "$CONTAINER_NAME" || true)
if [ "$EXISTS" -gt 0 ] && [ "$REBUILD" = true ]; then
  run distrobox rm --force "$CONTAINER_NAME"
  EXISTS=0
fi

if [ "$EXISTS" -eq 0 ]; then
  # Build the additional-flags string conditionally
  NVIDIA_FLAGS=""
  DEVICE_FLAGS="--device /dev/uinput"

  # Detect NVIDIA GPU on host and add device flags
  if nvidia-smi -L &>/dev/null; then
    NVIDIA_FLAGS="--nvidia"
    DEVICE_FLAGS="$DEVICE_FLAGS --device /dev/nvidia0 --device /dev/nvidiactl --device /dev/nvidia-uvm"
  fi

  run distrobox create \
    --name "$CONTAINER_NAME" \
    --image localhost/spellcast-dev:latest \
    $NVIDIA_FLAGS \
    $DEVICE_FLAGS \
    --volume /run/user/"$(id -u)"/pipewire-0:/run/user/"$(id -u)"/pipewire-0

  echo "  Container created."
else
  echo "  Container '$CONTAINER_NAME' already exists."
fi
echo ""

# Step 3 — Verify from inside
echo "--- Verifying from inside container ---"
verify() {
  run distrobox enter "$CONTAINER_NAME" -- bash -c "$*"
}

verify "echo 'uinput:'; ls -la /dev/uinput 2>/dev/null || echo '  MISSING'"
verify "echo 'nvidia:'; nvidia-smi --query-gpu=name --format=csv,noheader 2>/dev/null || echo '  not available'"
verify "echo 'audio:'; pw-record --version 2>/dev/null || echo '  pw-record not installed'"
verify "echo 'rust:'; rustc --version"
verify "echo 'cargo:'; cargo --version"
echo ""

echo "=== Done ==="
echo "  Enter:  distrobox enter $CONTAINER_NAME"
echo "  Build:  cd $PROJECT_DIR && cargo build"