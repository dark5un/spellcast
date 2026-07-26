// SPDX-License-Identifier: Apache-2.0

//! # Spellcast
//!
//! Spellcast is a dictation-first terminal keyboard multiplexer for Linux.
//! It provides token-aware speech-to-text with inline editing, phonetic prediction,
//! and a concept-to-word "explain" feature.
//!
//! ## Architecture
//!
//! Spellcast sits between the user and their shell using a PTY wrapper:
//!
//! ```text
//! ┌─────────────────────────────────────────────────────┐
//! │  Spellcast (PTY Wrapper)                               │
//! │  ┌──────────────────────────────────────────────┐  │
//! │  │  Status Bar: [DICT] token | 1:word 2:word   │  │
//! │  └──────────────────────────────────────────────┘  │
//! │  ┌──────────────────────────────────────────────┐  │
//! │  │  Shell (bash/zsh) running in PTY             │  │
//! │  └──────────────────────────────────────────────┘  │
//! ├─────────────────────────────────────────────────────┤
//! │  Modes: Dictation | Raw (Caps Lock toggle)          │
//! │  Kill Switch: Ctrl+Alt+X                            │
//! └─────────────────────────────────────────────────────┘
//! ```
//!
//! Key subsystems:
//! - **Audio**: Capture from microphone via `cpal`
//! - **ASR**: Speech-to-text via `whisper-rs` (whisper.cpp bindings)
//! - **Tokenizer**: Heuristic tokenization (prose vs code context)
//! - **Predictor**: Phonetic similarity via Double Metaphone
//! - **Explainer**: Concept-to-token via DB → LLM → web search
//! - **Memory**: Persistent SQLite database
//! - **Terminal**: PTY wrapper + status bar rendering

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
