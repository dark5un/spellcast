# Changelog

All notable changes to Spellcast are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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