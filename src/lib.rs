// SPDX-License-Identifier: Apache-2.0

//! # Spellcast
//!
//! Spellcast is a dictation-first terminal keyboard multiplexer for Linux.
//! It provides token-aware speech-to-text with inline editing, phonetic prediction,
//! and a concept-to-word "explain" feature.
//!
//! ## Architecture
//!
//! Spellcast sits between the user and their shell using a PTY wrapper.
//! The PTY reader runs on a background thread (mpsc channel); ASR also runs
//! on a background thread. Logs go to `~/.config/spellcast/spellcast.log`.
//!
//! Layout:
//! - Spellcast (PTY Wrapper)
//!   - Status Bar: `[DICT] token | 1:word 2:word`
//!   - Shell (bash/zsh) running in PTY
//! - Modes: Dictation | Raw (Ctrl+Space toggle)
//! - Kill Switch: Ctrl+G
//!
//! Key subsystems:
//! - **Audio**: Capture from microphone via `cpal` (PipeWire backend)
//! - **VAD**: Voice activity detection via `silero` crate (bundled ONNX model)
//! - **ASR**: Speech-to-text via `whisper-rs` (whisper.cpp bindings)
//! - **Tokenizer**: Heuristic and tree-sitter tokenization (prose vs code context)
//! - **Predictor**: Phonetic similarity via Double Metaphone (`rphonetic`)
//! - **Explainer**: Concept-to-token via DB → LLM → web search (LLM path not yet wired to event loop)
//! - **Memory**: Persistent SQLite database
//! - **Terminal**: PTY wrapper + status bar (raw ANSI escapes, not a TUI framework)
//!
//! ## Feature Model
//!
//! All core functionality is always compiled — no optional feature flags for
//! tree-sitter, VAD, LLM, or ASR. Only GPU backends are compile-time features:
//! `cuda` (default), `vulkan`, `cpu`. The runtime backend choice is a config/CLI option.

pub mod accessibility;
pub mod asr;
pub mod audio;
pub mod backend;
pub mod config;
pub mod error;
pub mod explainer;
pub mod macros;
pub mod memory;
pub mod modes;
pub mod navigation;
pub mod plugin;
pub mod predictor;
pub mod terminal;
pub mod tokenizer;

pub use config::SpellcastConfig;
pub use error::SpellcastError;
pub use modes::{Mode, ModeController};
