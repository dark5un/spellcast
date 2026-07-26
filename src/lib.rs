// SPDX-License-Identifier: Apache-2.0

//! # VoxKey
//!
//! VoxKey is a dictation-first terminal keyboard multiplexer for Linux.
//! It provides token-aware speech-to-text with inline editing, phonetic prediction,
//! and a concept-to-word "explain" feature.
//!
//! ## Architecture
//!
//! VoxKey sits between the user and their shell using a PTY wrapper:
//!
//! ```text
//! ┌─────────────────────────────────────────────────────┐
//! │  VoxKey (PTY Wrapper)                               │
//! │  ┌──────────────────────────────────────────────┐  │
//! │  │  Status Bar: [DICT] token | 1:word 2:word   │  │
//! │  └──────────────────────────────────────────────┘  │
//! │  ┌──────────────────────────────────────────────┐  │
//! │  │  Shell (bash/zsh) running in PTY             │  │
//! │  └──────────────────────────────────────────────┘  │
//! ├─────────────────────────────────────────────────────┤
//! │  Modes: Dictation | Raw (Caps Lock toggle)         │
//! │  Kill Switch: Ctrl+Shift+Escape                    │
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

pub mod config;
pub mod error;
pub mod audio;
pub mod asr;
pub mod tokenizer;
pub mod predictor;
pub mod explainer;
pub mod memory;
pub mod terminal;
pub mod modes;
pub mod backend;

pub use error::VoxKeyError;
pub use config::VoxKeyConfig;
pub use modes::{Mode, ModeController};