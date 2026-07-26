# SPDX-License-Identifier: Apache-2.0
#
# spec file for spellcast
#
# Copyright (c) 2025 Panos
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#     http://www.apache.org/licenses/LICENSE-2.0

%global __cargo_default_features std

Name:           spellcast
Version:        0.1.0
Release:        1%{?dist}
Summary:        Dictation-first terminal keyboard multiplexer for Linux

License:        Apache-2.0
URL:            https://github.com/dark5un/spellcastv1
Source0:        %{url}/archive/v%{version}/spellcast-%{version}.tar.gz

# Rust edition 2024 requires at least Rust 1.85
BuildRequires:  rust >= 1.85
BuildRequires:  cargo >= 1.85
BuildRequires:  gcc
BuildRequires:  gcc-c++
BuildRequires:  cmake
BuildRequires:  pkgconfig
BuildRequires:  alsa-lib-devel
BuildRequires:  pipewire-devel
BuildRequires:  pipewire-alsa
BuildRequires:  pulseaudio-libs-devel
BuildRequires:  systemd-devel
BuildRequires:  libsqlite3x-devel
BuildRequires:  make

# Core runtime dependencies
Requires:       alsa-lib
Requires:       pipewire
Requires:       pipewire-alsa
Requires:       pulseaudio-libs
Requires:       libsqlite3x

# Optional: NVIDIA CUDA support
%global has_cuda 0
%{?!_with_cuda:%global _with_cuda 0}
%if %{_with_cuda}
BuildRequires:  cuda-toolkit
%endif

%description
Spellcast is a dictation-first terminal keyboard multiplexer for Linux that
lets you speak your commands, code, and prose instead of typing them. It
provides token-aware speech-to-text with inline editing, phonetic prediction,
and a concept-to-word "explain" feature.

Features:
- Two modes: Dictation (Caps Lock ON) and Raw passthrough (Caps Lock OFF)
- Token navigation with H/L keys
- Phonetic predictions ranked by phoneme distance
- Explain feature: describe a concept verbally, get the right token
- Kill switch (Ctrl+Shift+Escape) to immediately disable
- Local only — all processing runs on your machine, no cloud
- GPU acceleration with CUDA (NVIDIA) and CPU backends
- Persistent memory — learns from your corrections over time

%prep
%autosetup -n spellcast-%{version}

%build
%if %{_with_cuda}
%{cargo_build} --features cuda
%else
%{cargo_build} --features cpu
%endif

%install
%{cargo_install}

# Install default configuration file
install -d -m 0755 %{buildroot}%{_sysconfdir}/spellcast/
install -m 0644 config/default-config.toml %{buildroot}%{_sysconfdir}/spellcast/config.toml.example

%check
%{cargo_test}

%files
%license LICENSE
%doc README.md CHANGELOG.md
%{_bindir}/spellcast
%config(noreplace) %{_sysconfdir}/spellcast/config.toml.example

%changelog
* Mon Jul 26 2026 Panos <panos@onlyascii.io> - 0.1.0-1
- Initial package for Spellcast 0.1.0