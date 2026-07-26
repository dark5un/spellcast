# SPDX-License-Identifier: Apache-2.0
#
# Containerfile — Spellcast development environment.
#
# Builds a fedora:latest-based image with all dependencies for Spellcast.
# Designed to be used with podman inside a distrobox container, or standalone.
#
# Build:
#   podman build -t spellcast-dev -f Containerfile .
#
# Run standalone (no GPU, no audio, no uinput — limited):
#   podman run --rm -it spellcast-dev
#
# Run with host integration (distrobox-style):
#   podman run --rm -it \
#     --device /dev/uinput \
#     --device /dev/nvidia0 --device /dev/nvidiactl --device /dev/nvidia-uvm \
#     --group-add keep-groups \
#     --security-opt label=disable \
#     --volume /run/user/$(id -u)/pipewire-0:/run/user/$(id -u)/pipewire-0:ro \
#     --volume "$HOME:$HOME" \
#     --workdir "$(pwd)" \
#     spellcast-dev
#
# NOTE: Audio access inside containers is limited. This Containerfile installs
# ALSA + PipeWire libs but the container needs the host's PipeWire socket at
# runtime (bind-mounted above). Without it, audio recording via cpal/ALSA will
# not work. For full audio + GPU + uinput passthrough, use distrobox instead:
#   distrobox create --name spellcast-dev --image spellcast-dev --nvidia
#
# This image is optimised for CUDA (NVIDIA). CPU-only builds omit the CUDA
# layers — simply remove or comment the CUDA-related RUN lines.

FROM fedora:latest

LABEL maintainer="Spellcast Team"
LABEL description="Spellcast development environment"
LABEL io.distrobox.image="spellcast-dev"

# Prevent interactive prompts during package installs
ENV DEBIAN_FRONTEND=noninteractive

# Install system dependencies
RUN dnf upgrade -y && \
    dnf install -y \
        # Build toolchain
        gcc \
        gcc-c++ \
        make \
        cmake \
        pkg-config \
        findutils \
        clang \
        # Audio libraries (ALSA backend for cpal, PipeWire for runtime)
        alsa-lib-devel \
        pipewire-devel \
        pipewire-alsa \
        pulseaudio-libs-devel \
        # SQLite (bundled via rusqlite, but headers for FFI)
        libsqlite3x-devel \
        # System headers
        systemd-devel \
        # Git and utilities
        git \
        curl \
        wget \
        sudo \
        # Runtime verification tools
        nvidia-smi \
        vulkan-tools \
        pipewire-utils \
        # CUDA toolkit (install from RPM Fusion / NVIDIA repo)
        cuda-toolkit \
    && dnf clean all

# Install Rust toolchain via rustup
ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH

RUN curl --proto =https --tlsv1.2 -sSf https://sh.rustup.rs | \
    sh -s -- -y --default-toolchain stable --no-modify-path && \
    rustup component add rustfmt clippy rust-analyzer && \
    rustup update

# Verify installations
RUN gcc --version && \
    cmake --version && \
    rustc --version && \
    cargo --version && \
    nvidia-smi -L 2>/dev/null || echo "(nvidia-smi only works at runtime)"

# Default command: run a shell
CMD ["/bin/bash"]