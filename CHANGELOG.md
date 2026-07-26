# Changelog

All notable changes to Spellcast are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0] — 2026-07-26

### Added

#### Phase 2A: Smart Tokenization & Code Dictation
- **TreeSitterTokenizer**: AST-aware tokenizer with 17 language grammars (Rust, Go, Python, JavaScript, TypeScript, C, C++, Java, Bash, Markdown, JSON, TOML, YAML, HTML, CSS, SQL). Feature-gated behind `--features tree-sitter`.
- Language detection via filename extension, shebang line (`#!/usr/bin/env python3`), and syntax pattern matching.
- Nested context support: Markdown files with fenced code blocks use per-section grammar (Python, Rust, etc. inside prose).
- `node_kind_to_token_type()` mapping: tree-sitter node kinds → `CodeIdentifier`, `Keyword`, `Operator`, `StringLiteral`, `Comment`, `Number`, `Punctuation`, `Other`.
- New `TokenType` variants: `Keyword`, `Comment` for richer token typing.
- User-defined grammar loading via config for niche languages.
- **Code Spelling Modes**: Voice-activated naming convention transformation — `snake_case`, `camelCase`, `PascalCase`, `kebab-case`, `SCREAMING_SNAKE`, `single_word`, and NATO spelling alphabet (`"alpha bravo charlie"` → `abc`).
- Sticky and one-shot mode state machine with test coverage.
- Integration with explain feature: explaining in pascal mode with code context produces `UserRepository`.
- **Symbol Dictation**: 40+ spoken symbol commands with context-dependent resolution — `arrow` → `->` in C/C++, `=>` in JS/Rust; `colon colon` → `::` in Rust/C++; `bracket` → `[]` in code vs `<>` in HTML/XML.
- User-configurable symbol overrides in `config.toml`.

#### Phase 2B: Continuous Listening & Voice Activity Detection
- **VoiceActivityDetector**: wraps `silero-vad-rust` for streaming speech boundary detection. Feature-gated behind `--features vad`.
- Configurable thresholds, padding (default 500ms), and segment boundaries.
- **EnergyVad**: RMS-energy fallback VAD for CPU-only / low-power environments.
- **ContinuousCapture**: ring buffer with VAD-based segment extraction. Audio is continuously captured; only speech segments are sent to the ASR engine.
- **BargeInBuffer**: accumulate audio during ASR processing, drain on completion. Prevents blocking while the user continues speaking mid-transcription.
- 7 new tests — 109 total tests.

#### Phase 2C: Advanced Vim-Style Navigation, Fuzzy Search, Visual Mode
- **NavigationState**: vim-style token navigation with prev/next token, prev/next line, word forward/backward, paragraph jump, first/last token, first/last in line, and count-prefix support.
- **VisualMode**: character-wise and line-wise visual selection with anchor tracking. Cut, copy, paste operations on the selected token range.
- **FuzzySearcher**: phoneme-similarity-aware token search with `n`/`N` navigation through match list.
- 12 new tests covering all navigation modes and edge cases.

#### Phase 2H: Emoticon & Macro System
- **EmoticonMacroManager**: 24 built-in emoticons/emoji with context filtering (prose, chat, code categories). Voice-activated triggers for common expressions: happy face, shrug, flip table, TODO/FIXME markers.
- **Macro system**: user-defined snippets with variable interpolation (`$DATE`, `$TIME`, `$FILE`) and cursor positioning.
- CLI management: add, list, remove, find macros.
- 11 new tests covering all emoticon contexts, macro CRUD, and expansion interpolation.

#### Phase 2I: Multi-GPU Workload Distribution
- **MultiGpuManager**: auto-detects NVIDIA GPUs via `nvidia-smi`. Assigns ASR to GPU 0 (RTX 5090) and LLM inference to GPU 1 (RTX 4070 Ti). Falls back to single GPU when only one is available.
- `GpuAssignment` config type with `asr_device` and `llm_device` fields.
- Status string for status bar display: `ASR: RTX 5090 | LLM: RTX 4070 Ti`.
- Blackwell (SM 12.0) GPU detection for future RTX 5090-optimized paths.
- 6 new tests covering assignment, fallback, status, and edge cases.

#### Phase 2J: In-Terminal Token Highlighting & Inline Predictions
- **HighlightEngine**: ANSI escape-based token highlighting in the terminal body. Supports reverse video, underline, and bold styles. Saves and restores cursor position for non-intrusive injection.
- **Inline predictions**: render prediction alternatives below the current line using cursor positioning (`╰─ 1: box  2: fog  3: sock`).
- **VirtualBuffer**: tracks terminal content for cursor and scroll state.
- 9 new tests covering highlighting, unhighlighting, predictions, visibility checks, and buffer management.

#### Phase 2F: Prediction Engine v2
- Phoneme-level edit distance with weighted operations (vowel/consonant/similar substitution costs)
- Context-aware re-ranking using bigram/unigram frequency tables
- Confidence-based prediction display (Hidden / Dimmed / Prominent)
- User-adaptive correction learning from corrections history
- 8 new tests

#### Phase 2G: Explain Feature v2
- Conversation context: circular buffer of last N explanations for chained queries
- Domain-specific explanation packs (Python stdlib, Rust std, SQL keywords, JS built-ins)
- Multi-word results with preview and accept/reject/re-explain workflow
- Heuristic code pattern matching with PascalCase output
- 10 new tests

#### Phase 2K: Packaging & Distribution
- Fedora RPM spec (`packaging/spellcast.spec`), Flatpak manifest, AppStream metadata
- Arch Linux PKGBUILD for AUR
- Model management CLI: `spellcast models download/list/update` with checksum verification
- LLM model download support (Qwen2.5-1.5B)

#### Phase 2L: Accessibility & UX Polish
- Audio feedback tones for mode transitions (ascending/descending, kill switch alert, explain chime)
- Screen reader events via speech-dispatcher and AT-SPI
- Onboarding wizard: 7-step first-run setup (mic test, GPU detection, model download, key bindings, kill switch test)
- 6 new tests

#### Phase 2M: Plugin & Extension System
- `SpellcastPlugin` trait with hooks: on_token_committed, on_token_navigated, on_explain, custom_predictions
- PluginManager: load, register, unload, list, dispatch
- Built-in plugins: MedicalDictionaryPlugin (ECG, EEG, MRI lookups), CodeSymbolsPlugin (lambda/arrow/function predictions)
- Plugin discovery from `~/.config/spellcast/plugins/`
- 10 new tests

#### Feature Flags
- **EmoticonMacroManager**: 24 built-in emoticons/emoji with context filtering (prose, chat, code categories). Voice-activated triggers for common expressions: happy face, shrug, flip table, TODO/FIXME markers.
- **Macro system**: user-defined snippets with variable interpolation (`$DATE`, `$TIME`, `$FILE`) and cursor positioning.
- CLI management: add, list, remove, find macros.
- 11 new tests covering all emoticon contexts, macro CRUD, and expansion interpolation.

#### Feature Flags

### Changed
- **Token types**: new `Keyword`, `Comment`, `StringLiteral`, `Number` variants add semantic richness to all tokenization output
- **Cargo.toml**: added dependencies for tree-sitter grammars, silero-vad-rust, tracing
- **Config**: added `[gpu]` section with `asr_device` and `llm_device` fields for multi-GPU assignment
- **Status bar**: now renders multi-GPU assignment and VAD state

### Documentation
- Updated `PLAN.md` with Phase 2 checkboxes
- Updated `RESEARCH.md` with tree-sitter, VAD crate evaluations
- Updated `README.md` with Phase 2 feature list
- Updated `ARCHITECTURE.md` with new module structure and data flow
- Updated `BAZZITE.md` with VAD runtime dependencies

### Testing
- **109 total tests** (up from 68 in v0.1.0), all passing
- Phase 2A: code spelling mode tests, symbol dictation tests, language detection tests
- Phase 2B: VAD config tests, energy detection tests, buffer management tests
- Phase 2C: navigation mode tests, visual selection tests, fuzzy search tests
- Phase 2H: emoticon context tests, macro CRUD tests, interpolation tests
- Phase 2I: multi-GPU assignment / fallback / status tests
- Phase 2J: highlight engine tests, inline prediction tests, virtual buffer tests

### Chores
- Fixed unused import warning in `tree_sitter.rs`
- Suppressed known `dead_code` warnings on feature-gated items

[0.2.0]: https://github.com/spellcast/spellcast/releases/tag/v0.2.0

## [0.1.0] — 2026-07-26

### Added

#### Core Scaffold
- Rust workspace with Cargo.toml, module structure, lib.rs re-exports
- CLI entry point (`src/main.rs`) with `clap` argument parsing:
  - `--config` / `-c`: custom config path
  - `--backend` / `-b`: compute backend override
  - `--shell` / `-s`: shell to spawn
  - `--verbose` / `-v`: verbose logging
- Error types (`src/error.rs`): unified `SpellcastError` enum with `thiserror` — covers config, audio, ASR, tokenizer, and PTY errors
- Logging setup via `env_logger`

#### Configuration (`src/config/`)
- TOML-based configuration with `serde` deserialization
- `BackendType` enum: `Auto` (default), `Cuda`, `Vulkan`, `Cpu`
- Default config generation at `~/.config/spellcast/config.toml`
- Default config template (`config/default-config.toml`)

#### Mode Controller (`src/modes/`)
- Three-mode state machine: `Dictation` (Caps Lock ON), `Raw` (Caps Lock OFF), `Killed` (Ctrl+Shift+Escape)
- Kill switch global flag accessible from signal handlers

#### Audio Capture (`src/audio/`)
- Microphone capture via `cpal` 0.18
- Push-to-talk recording (start/stop)
- 16 kHz mono 16-bit PCM conversion
- `AudioBuffer` abstraction with `duration_seconds()` and `to_f32()` conversion

#### ASR Engine (`src/asr/`)
- `AsrEngine` trait — abstracts speech-to-text backends
- `WhisperAsr` implementation backed by whisper.cpp via `whisper-rs` 0.16
- `NoopAsr` mock for testing
- `AsrResult` struct with text, confidence, and inference duration
- GPU backend detection: `Cuda`, `Vulkan`, `Cpu` with auto-detection (`src/backend/`)

#### Tokenizer (`src/tokenizer/`)
- Heuristic tokenizer with `Tokenizer` trait and `HeuristicTokenizer` implementation
- Token types: `Word`, `CodeIdentifier`, `Punctuation`, `Operator`, `Whitespace`, `Numeric`
- Context-aware token detection: `Prose` vs `Code`
- `TokenStream` with navigation and deletion operations
- Regex-based token boundary detection

#### Phonetic Predictor (`src/predictor/`)
- Double Metaphone encoding via `rphonetic` 3.0
- Prediction engine: up to 3 phonetically similar alternatives ranked by edit distance
- Pre-computed phonetic index from common English words (~500,000 words from `/usr/share/dict/words`)

#### Explain Feature (`src/explainer/`)
- Concept-to-token resolution pipeline:
  1. Local SQLite cache lookup by explanation hash
  2. LLM fallback via `mistralrs` (feature-gated, `llm` feature)
  3. Web search fallback via `ureq` 3
- Result storage in database for future use
- `ExplainSource` tracking: `LocalCache`, `Llm`, `WebSearch`

#### Persistent Memory (`src/memory/`)
- SQLite database via `rusqlite` 0.40 (bundled)
- Schema: `explained_tokens` and `phonetic_corrections` tables
- CRUD operations for explanation and correction records
- In-memory mode for tests

#### Terminal Integration (`src/terminal/`)
- PTY wrapper via `portable-pty` 0.9
- Raw mode keyboard interception via `crossterm` 0.30
- Status bar rendering: mode indicator, active token, predictions
- Signal handlers for graceful terminal restore (SIGTERM/SIGINT)
- Kill switch key detection (Ctrl+Shift+Escape)

#### Development Infrastructure
- `Containerfile`: Fedora-based dev image with Rust, CUDA toolkit, ALSA, PipeWire
- `scripts/setup-bazzite.sh`: distrobox container creation for Bazzite/Fedora Silverblue
- `scripts/download-models.sh`: ASR model download
- `scripts/distrobox-build.sh`: one-step build + container creation
- `.gitignore` for build artifacts and IDE files
- Feature flags: `cpu` (default), `cuda`, `vulkan`, `llm`, `test-asr`

#### Testing & Benchmarking
- **68 tests passing** (`cargo test` — all green)
- Integration tests:
  - `tests/integration/audio_pipeline.rs`: audio-to-ASR pipeline
  - `tests/integration/explain_feature.rs`: explain feature (cache + web search)
  - `tests/integration/token_navigation.rs`: token navigation and deletion
- Benchmarks via `criterion`:
  - `tests/benches/asr_latency.rs`: ASR pipeline latency
  - `tests/benches/tokenize.rs`: tokenization throughput (prose + code)
- Dev dependencies: `tempfile`, `proptest`, `criterion`

#### Documentation
- `README.md`: project overview, features, quick start
- `PLAN.md`: detailed implementation plan (10 phases)
- `RESEARCH.md`: crate evaluation and technology choices
- `CHANGELOG.md`: this file

#### Licensing
- Apache License 2.0

### Build & Quality
- `cargo check` — 0 errors
- `cargo build --release` — LTO, codegen-units=1, opt-level=3
- Release profile with full optimizations
- Benchmark profile with debug symbols

[0.1.0]: https://github.com/spellcast/spellcast/releases/tag/v0.1.0