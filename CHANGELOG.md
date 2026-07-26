# Changelog

All notable changes to Spellcast are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

#### Feature Model Overhaul
- **Removed optional feature flags** for tree-sitter, VAD, LLM, and ASR — everything is always compiled now.
- Only GPU backend selection remains as compile-time features: `cuda` (default), `vulkan`, `cpu`.
- Backend choice (`auto`/`cuda`/`vulkan`/`cpu`) is a runtime config/CLI option, not a compile-time choice.
- `ratatui` removed from dependencies (was unused).
- `mistralrs` now non-optional (always compiled, v0.8.1).
- `silero` crate (v0.6.0) replaces `silero-vad-rust` for VAD. Bundled ONNX model — no separate download.
- Tree-sitter grammar crates updated: `tree-sitter-toml-ng`, `tree-sitter-sequel` (SQL), `tree-sitter-md-025` (Markdown).
- `cpal` uses PipeWire backend (not ALSA): `cpal = { version = "0.18.1", features = ["pipewire"] }`.

#### Keybindings
- **Mode toggle**: Ctrl+Space (universal). Caps Lock also works if terminal supports kitty keyboard protocol.
- **Kill switch**: Ctrl+G (not Ctrl+Shift+Escape). Both Ctrl+G and BEL (0x07) are detected in raw mode.
- **Dictation mode**: VAD-based continuous listening is the design goal (not push-to-talk). Currently, Space triggers a 3-second push-to-talk recording.
- `h`/`l`: navigate tokens, `x`: delete, `r`: re-dictate, `e`: explain, `1`-`3`: accept predictions.
- Space/Enter/Backspace: standard keys, pass through to PTY.

#### Architecture
- PTY reader runs on a **background thread** with `mpsc::channel<Vec<u8>>` (not blocking read). Main loop drains non-blocking via `try_recv`.
- ASR runs on a **background thread**, results via `mpsc::channel<Result<String, String>>`. `asr_busy` flag prevents concurrent recordings.
- **Logs go to `~/.config/spellcast/spellcast.log`** (not stderr) to avoid corrupting the terminal display.
- **whisper.cpp stdout/stderr suppressed** during model load via `dup2` to `/dev/null` in `main.rs`.
- **Status bar uses raw ANSI escapes** (not ratatui). Cursor save/restore + reverse video on bottom row.
- File paths: `src/plugin/mod.rs` (not `src/plugins/`), `src/accessibility.rs` (not `src/accessibility/`).
- Onboarding has **8 steps** (not 7): Welcome, MicrophoneTest, GpuDetection, ModelDownload, DictationTest, KeyBindings, KillSwitch, Complete.

#### CLI
- `--check-audio`: lists input devices only (numbered, deduplicated, marks default with `*`).
- `--set-input-device`: saves device name to config and exits.
- `-v`: verbose (debug) logging to file.
- No `models` or `plugins` subcommands exist (removed from docs if previously claimed).

#### Explain Feature
- DB lookup works: `MemoryStore::lookup_explained` computes hash internally — no double-hashing by `Explainer`.
- LLM path code exists (`mistralrs` with Qwen/Qwen3-4B, 4-bit ISQ) but is **not yet wired into the event loop**. Pressing `e` in dictation mode logs "explainer not yet wired".

#### Config
- `[keys]` section `kill_switch` default updated to `"Ctrl+G"` to match the actual terminal event loop behavior. Config-driven keybinding is a planned improvement.
- `[llm]` model_path default updated to `Qwen/Qwen3-4B`.

### Fixed
- Fixed duplicate "Feature Flags" sections in changelog (was appearing twice).
- Removed all "VoxKey" references (project renamed to Spellcast).

---

## [0.2.0] — 2026-07-26

### Added

#### Phase 2A: Smart Tokenization & Code Dictation
- **TreeSitterTokenizer**: AST-aware tokenizer with 17 language grammars (Rust, Go, Python, JavaScript, TypeScript, C, C++, Java, Bash, Markdown, JSON, TOML, YAML, HTML, CSS, SQL). Always compiled (no feature flag).
- Language detection via filename extension, shebang line, and syntax pattern matching.
- Nested context support: Markdown files with fenced code blocks use per-section grammar.
- `node_kind_to_token_type()` mapping: tree-sitter node kinds → `CodeIdentifier`, `Keyword`, `Operator`, `StringLiteral`, `Comment`, `Number`, `Punctuation`, `Other`.
- New `TokenType` variants: `Keyword`, `Comment` for richer token typing.
- User-defined grammar loading via config for niche languages.
- **Code Spelling Modes**: Voice-activated naming convention transformation — `snake_case`, `camelCase`, `PascalCase`, `kebab-case`, `SCREAMING_SNAKE`, `single_word`, and NATO spelling alphabet.
- Sticky and one-shot mode state machine with test coverage.
- Integration with explain feature: explaining in pascal mode with code context produces `UserRepository`.
- **Symbol Dictation**: 40+ spoken symbol commands with context-dependent resolution — `arrow` → `->` in C/C++, `=>` in JS/Rust; `colon colon` → `::` in Rust/C++; `bracket` → `[]` in code vs `<>` in HTML/XML.
- User-configurable symbol overrides in `config.toml`.

#### Phase 2B: Continuous Listening & Voice Activity Detection
- **VoiceActivityDetector**: wraps `silero` crate v0.6.0 for streaming speech boundary detection. Always compiled (no feature flag). Bundled ONNX model.
- Configurable thresholds, padding (default 500ms), and segment boundaries.
- **EnergyVad**: RMS-energy fallback VAD for CPU-only / low-power environments.
- **ContinuousCapture**: ring buffer with VAD-based segment extraction. Audio is continuously captured; only speech segments are sent to the ASR engine.
- **BargeInBuffer**: accumulate audio during ASR processing, drain on completion.

#### Phase 2C: Advanced Vim-Style Navigation, Fuzzy Search, Visual Mode
- **NavigationState**: vim-style token navigation with prev/next token, prev/next line, word forward/backward, paragraph jump, first/last token, first/last in line, and count-prefix support.
- **VisualMode**: character-wise and line-wise visual selection with anchor tracking. Cut, copy, paste operations on the selected token range.
- **FuzzySearcher**: phoneme-similarity-aware token search with `n`/`N` navigation through match list.

#### Phase 2F: Prediction Engine v2
- Phoneme-level edit distance with weighted operations (vowel/consonant/similar substitution costs)
- Context-aware re-ranking using bigram/unigram frequency tables
- Confidence-based prediction display (Hidden / Dimmed / Prominent)
- User-adaptive correction learning from corrections history

#### Phase 2G: Explain Feature v2
- Conversation context: circular buffer of last N explanations for chained queries
- Domain-specific explanation packs (Python stdlib, Rust std, SQL keywords, JS built-ins)
- Multi-word results with preview and accept/reject/re-explain workflow
- Heuristic code pattern matching with PascalCase output

#### Phase 2H: Emoticon & Macro System
- **EmoticonMacroManager**: 24 built-in emoticons/emoji with context filtering (prose, chat, code categories).
- **Macro system**: user-defined snippets with variable interpolation (`$DATE`, `$TIME`, `$FILE`) and cursor positioning.

#### Phase 2I: Multi-GPU Workload Distribution
- **MultiGpuManager**: auto-detects NVIDIA GPUs via `nvidia-smi`. Assigns ASR to GPU 0 and LLM inference to GPU 1. Falls back to single GPU when only one is available.
- `GpuAssignment` config type with `asr_device` and `llm_device` fields.
- Blackwell (SM 12.0) GPU detection for future RTX 5090-optimized paths.

#### Phase 2J: In-Terminal Token Highlighting & Inline Predictions
- **HighlightEngine**: ANSI escape-based token highlighting in the terminal body.
- **Inline predictions**: render prediction alternatives below the current line.
- **VirtualBuffer**: tracks terminal content for cursor and scroll state.

#### Phase 2L: Accessibility & UX Polish
- Audio feedback tones for mode transitions (ascending/descending, kill switch alert, explain chime)
- Screen reader events via speech-dispatcher (`spd-say`)
- Onboarding wizard: 8-step first-run setup (Welcome, MicrophoneTest, GpuDetection, ModelDownload, DictationTest, KeyBindings, KillSwitch, Complete)

#### Phase 2M: Plugin & Extension System
- `SpellcastPlugin` trait with hooks: on_token_committed, on_token_navigated, on_explain, custom_predictions
- PluginManager: register, unload, list, dispatch
- Built-in plugins: MedicalDictionaryPlugin (ECG, EEG, MRI lookups), CodeSymbolsPlugin (lambda/arrow/function predictions)
- Plugin discovery from `~/.config/spellcast/plugins/` (planned; not yet wired into event loop)

### Changed
- **Token types**: new `Keyword`, `Comment`, `StringLiteral`, `Number` variants add semantic richness to all tokenization output
- **Cargo.toml**: tree-sitter grammars, silero VAD, mistralrs are always compiled (no feature flags)
- **Config**: `[backend]` section with `gpu_assignment` for multi-GPU assignment

---

## [0.1.0] — 2026-07-26

### Added

#### Core Scaffold
- Rust workspace with Cargo.toml, module structure, lib.rs re-exports
- CLI entry point (`src/main.rs`) with `clap` argument parsing:
  - `--config` / `-c`: custom config path
  - `--backend` / `-b`: compute backend override
  - `--shell` / `-s`: shell to spawn
  - `--verbose` / `-v`: verbose logging
  - `--check-audio`: list input devices
  - `--set-input-device`: save device to config
- Error types (`src/error.rs`): unified `SpellcastError` enum with `thiserror`
- Logging to file (`~/.config/spellcast/spellcast.log`) via `env_logger`

#### Configuration (`src/config/`)
- TOML-based configuration with `serde` deserialization
- `BackendType` enum: `Auto` (default), `Cuda`, `Vulkan`, `Cpu`
- Default config generation at `~/.config/spellcast/config.toml`
- Default config template (`config/default-config.toml`)

#### Mode Controller (`src/modes/`)
- Three-mode state machine: `Dictation`, `Raw`, `Killed`
- Mode toggle: Ctrl+Space (universal) or Caps Lock (kitty protocol)
- Kill switch: Ctrl+G (detects both Ctrl+G and BEL 0x07 in raw mode)
- Kill switch global flag accessible from signal handlers

#### Audio Capture (`src/audio/`)
- Microphone capture via `cpal` 0.18 (PipeWire backend)
- `record_duration(secs)` for fixed-duration recording
- `start_continuous(chunk_tx)` for VAD-based streaming
- 16 kHz mono 16-bit PCM conversion
- `AudioBuffer` abstraction with `duration_seconds()` and `to_f32()` conversion

#### Voice Activity Detection (`src/audio/vad.rs`)
- `VoiceActivityDetector` wrapping `silero` crate v0.6.0 (bundled ONNX model)
- `EnergyVad` fallback for CPU-only environments
- `ContinuousCapture` ring buffer with VAD segment extraction
- `BargeInBuffer` for accumulating audio during ASR processing

#### ASR Engine (`src/asr/`)
- `AsrEngine` trait — abstracts speech-to-text backends
- `WhisperAsr` implementation backed by whisper.cpp via `whisper-rs` 0.16
- `NoopAsr` mock for testing
- `AsrResult` struct with text, confidence, and inference duration
- whisper.cpp stdout/stderr suppressed during model load (dup2)
- GPU backend detection: `Cuda`, `Vulkan`, `Cpu` with auto-detection (`src/backend/`)

#### Tokenizer (`src/tokenizer/`)
- Heuristic tokenizer with `Tokenizer` trait and `HeuristicTokenizer` implementation
- Token types: `Word`, `CodeIdentifier`, `Keyword`, `Punctuation`, `Operator`, `Whitespace`, `Number`, `StringLiteral`, `Comment`, `Other`
- Context-aware token detection: `Prose` vs `Code`
- `TokenStream` with navigation and deletion operations
- Regex-based token boundary detection
- Tree-sitter tokenizer with 17+ language grammars (always compiled)

#### Phonetic Predictor (`src/predictor/`)
- Double Metaphone encoding via `rphonetic` 3.0
- Prediction engine: up to 3 phonetically similar alternatives ranked by edit distance
- Pre-computed phonetic index from common English words

#### Explain Feature (`src/explainer/`)
- Concept-to-token resolution pipeline:
  1. Local SQLite cache lookup (hash computed by MemoryStore)
  2. LLM fallback via `mistralrs` (always compiled, Qwen/Qwen3-4B)
  3. Web search fallback via `ureq` 3 (dictionary stub for MVP)
- Result storage in database for future use
- `ExplainSource` tracking: `LocalCache`, `Llm`, `WebSearch`

#### Persistent Memory (`src/memory/`)
- SQLite database via `rusqlite` 0.40 (bundled)
- Schema: `explained_tokens` and `phonetic_corrections` tables
- Hash computed internally by MemoryStore (no double-hashing)
- CRUD operations for explanation and correction records
- In-memory mode for tests

#### Terminal Integration (`src/terminal/`)
- PTY wrapper via `portable-pty` 0.9
- PTY reader on background thread with mpsc channel (non-blocking)
- ASR on background thread with mpsc channel
- Raw mode keyboard interception via `crossterm` 0.29
- Status bar rendering via raw ANSI escapes (not ratatui)
- Alternate screen mode
- Signal handlers for graceful terminal restore (SIGTERM/SIGINT)
- Kill switch key detection (Ctrl+G / BEL)

#### Development Infrastructure
- `Containerfile`: Fedora-based dev image with Rust, CUDA toolkit, PipeWire
- `scripts/setup-bazzite.sh`: distrobox container creation for Bazzite/Fedora Silverblue
- `scripts/download-models.sh`: ASR model download
- `scripts/distrobox-build.sh`: one-step build + container creation
- `.gitignore` for build artifacts and IDE files
- Compile-time features: `cuda` (default), `vulkan`, `cpu` (GPU SDK only)

#### Documentation
- `README.md`: project overview, features, quick start, CLI, keybindings
- `PLAN.md`: detailed implementation plan
- `RESEARCH.md`: crate evaluation and technology choices
- `CHANGELOG.md`: this file
- `docs/ARCHITECTURE.md`: system architecture and data flow
- `docs/BAZZITE.md`: Bazzite Linux setup guide

#### Licensing
- Apache License 2.0

### Build & Quality
- `cargo check` — 0 errors
- `cargo build --release` — LTO, codegen-units=1, opt-level=3
- Release profile with full optimizations
