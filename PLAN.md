# Spellcast — Implementation Plan

## Phase 0: Scaffold (Week 1)
- [x] Create project structure (Cargo.toml, directories)
- [x] Config types (`src/config/`) with TDD
- [x] Error types (`src/error.rs`) with TDD
- [x] CLI parsing (`src/main.rs`, `clap`)
- [x] Default config file (`config/default-config.toml`)
- [x] Logging setup (file-based: `~/.config/spellcast/spellcast.log`)
- [x] `setup-bazzite.sh` script
- [x] `download-models.sh` script
- [x] `cargo build` passes
- [x] `cargo clippy` passes
- [x] `cargo fmt --check` passes
- [x] Deliverable: Working binary that prints help and validates config

## Phase 1: Mode Controller + Config (Week 1-2)
- [x] Mode enum (`Dictation`, `Raw`, `Killed`)
- [x] Mode controller tests
- [x] Kill switch logic (Ctrl+G detection in terminal; config default updated to Ctrl+G)
- [x] Caps Lock toggle + Shift+Caps Lock for normal caps
- [x] Ctrl+Space as universal mode toggle (kitty protocol Caps Lock also supported)
- [x] TDD: All mode transitions tested
- [x] Deliverable: Mode state machine with tests

## Phase 2: Audio Capture (Week 2-3)
- [x] Audio capture module with `cpal` (PipeWire backend)
- [x] `record_duration(secs)` — fixed-duration recording
- [x] `start_continuous(chunk_tx)` — continuous streaming for VAD
- [x] 16kHz mono 16-bit PCM conversion
- [x] Audio buffer abstraction
- [x] TDD: Mock audio device, test buffer pipeline
- [x] Deliverable: Can record audio and produce PCM buffer

## Phase 3: ASR Integration (Week 3-4)
- [x] ASR trait with `whisper-rs` (v0.16.0)
- [x] Model loading (tiny/base models)
- [x] GPU backend detection (CUDA vs CPU) via `nvidia-smi`
- [x] Audio → text transcription
- [x] whisper.cpp stdout/stderr suppression during model load (dup2 to /dev/null)
- [x] TDD: Mock ASR engine (`NoopAsr`), test integration
- [x] Deliverable: Spoken words → text

## Phase 4: PTY Wrapper (Week 4-5)
- [x] PTY spawn shell via `portable-pty`
- [x] PTY reader on background thread with mpsc channel (non-blocking)
- [x] Keystroke interception
- [x] Text injection into PTY
- [x] Raw passthrough mode (Spellcast transparent)
- [x] Status bar via raw ANSI escapes (not ratatui — removed)
- [x] TDD: Mock PTY, test keystroke routing
- [x] Deliverable: Shell runs inside Spellcast, mode switching works

## Phase 5: Tokenizer (Week 5-6)
- [x] Token types (Word, CodeIdentifier, Keyword, Punctuation, Operator, Whitespace, Number, StringLiteral, Comment, Other)
- [x] Heuristic tokenizer (regex-based)
- [x] Context detection (prose vs code)
- [x] Token navigation (prev/next with h/l)
- [x] Token highlighting in status bar
- [x] Property-based tests with `proptest`
- [x] Deliverable: Text → token sequence, navigation works

## Phase 6: Phonetic Predictions (Week 6-7)
- [x] Phonetic encoding via `rphonetic` (Double Metaphone)
- [x] Prediction engine: top-3 phonetically similar tokens
- [x] Pre-built phonetic index from common English words
- [x] Prediction display in status bar
- [x] Accept prediction (keys 1/2/3)
- [x] TDD: Phonetic distance tests
- [x] Deliverable: After dictation, 3 predictions shown

## Phase 7: Memory/Learning DB (Week 7-8)
- [x] SQLite schema (`explained_tokens`, `phonetic_corrections`)
- [x] DB initialization and migrations
- [x] Insert/query explained tokens (hash computed internally — no double-hashing)
- [x] Insert/query phonetic corrections
- [x] TDD: DB operations tested with in-memory DB
- [x] Deliverable: Persistent memory across sessions

## Phase 8: Explain Feature (Week 8-9)
- [x] Explain: listen to explanation
- [x] Local DB lookup by explanation text (hash computed by MemoryStore)
- [x] LLM fallback via `mistralrs` (always compiled, Qwen/Qwen3-4B, 4-bit ISQ)
- [x] Web search fallback via `ureq` (dictionary stub for MVP)
- [x] Result storage in DB
- [x] TDD: Mock LLM, mock web, test fallback chain
- [ ] **TODO:** Wire explainer into terminal event loop (`e` key currently logs "not yet wired")
- [x] Deliverable: "Explain" produces correct token (in library; event loop wiring pending)

## Phase 9: Integration & Polish (Week 9-10)
- [x] End-to-end pipeline test
- [x] Config file loading
- [x] Latency benchmarks
- [x] Signal handling (SIGTERM, SIGINT)
- [x] Graceful terminal restore on panic
- [x] Logging to file (`~/.config/spellcast/spellcast.log`)
- [x] Documentation finalization
- [x] Deliverable: Working MVP binary

## Phase 10: Documentation & Packaging (Week 10)
- [x] README.md
- [x] ARCHITECTURE.md
- [x] BAZZITE.md
- [ ] CONFIGURATION.md
- [ ] DEVELOPMENT.md
- [ ] CONTRIBUTING.md
- [x] LICENSE (Apache 2.0)
- [x] CHANGELOG.md
- [ ] Final quality gates: build, test, clippy, fmt
- [x] Deliverable: Complete project ready for use

## Phase 2A: Smart Tokenization & Code Dictation
- [x] TreeSitterTokenizer: AST-aware tokenizer with 17 language grammars (always compiled)
- [x] Language detection via filename extension, shebang line, and syntax pattern matching
- [x] Nested context support: Markdown files with fenced code blocks
- [x] `node_kind_to_token_type()` mapping
- [x] Code Spelling Modes: snake_case, camelCase, PascalCase, kebab-case, SCREAMING_SNAKE, single_word, NATO
- [x] Symbol Dictation: 40+ spoken symbol commands with context-dependent resolution
- [x] Deliverable: Smart tokenization with code spelling and symbol dictation

## Phase 2B: Continuous Listening & Voice Activity Detection
- [x] VoiceActivityDetector: wraps `silero` crate v0.6.0 (bundled ONNX model, no feature flag)
- [x] Configurable thresholds, padding, segment boundaries
- [x] EnergyVad: RMS-energy fallback VAD for CPU-only environments
- [x] ContinuousCapture: ring buffer with VAD-based segment extraction
- [x] BargeInBuffer: accumulate audio during ASR processing
- [ ] **TODO:** Wire VAD continuous capture into the terminal event loop (currently using push-to-talk)
- [x] Deliverable: VAD module functional (wiring to event loop pending)

## Phase 2C: Advanced Vim-Style Navigation, Fuzzy Search, Visual Mode
- [x] NavigationState: vim-style token navigation with count-prefix support
- [x] VisualMode: character-wise and line-wise visual selection
- [x] FuzzySearcher: phoneme-similarity-aware token search with n/N navigation
- [x] Deliverable: Advanced navigation module (`src/navigation.rs`)

## Phase 2I: Multi-GPU Workload Distribution
- [x] MultiGpuManager: auto-detects NVIDIA GPUs via `nvidia-smi`
- [x] GpuAssignment config type with asr_device and llm_device fields
- [x] Blackwell (SM 12.0) GPU detection
- [x] Deliverable: Multi-GPU support (`src/backend/multi_gpu.rs`)

## Phase 2J: In-Terminal Token Highlighting & Inline Predictions
- [x] HighlightEngine: ANSI escape-based token highlighting
- [x] Inline predictions: render alternatives below current line
- [x] VirtualBuffer: tracks terminal content for cursor and scroll state
- [x] Deliverable: Highlight engine (`src/terminal/highlight.rs`)

## Phase 2F: Prediction Engine v2
- [x] Phoneme-level edit distance with weighted operations
- [x] Context-aware re-ranking using bigram/unigram frequency tables
- [x] Confidence-based prediction display
- [x] User-adaptive correction learning
- [x] Deliverable: Prediction engine v2 (`src/predictor/v2.rs`)

## Phase 2G: Explain Feature v2
- [x] Conversation context: circular buffer of last N explanations
- [x] Domain-specific explanation packs
- [x] Multi-word results with preview and accept/reject/re-explain workflow
- [x] Heuristic code pattern matching with PascalCase output
- [x] Deliverable: Explain v2 (`src/explainer/v2.rs`)

## Phase 2L: Accessibility & UX Polish
- [x] Audio feedback module — beep tones for mode transitions
- [x] Screen reader events via `spd-say`
- [x] Onboarding wizard — 8-step first-run setup (Welcome, MicrophoneTest, GpuDetection, ModelDownload, DictationTest, KeyBindings, KillSwitch, Complete)
- [x] Deliverable: Accessibility module (`src/accessibility.rs`)
- [ ] **TODO:** Wire onboarding wizard into main event loop (first-run detection)

## Phase 2M: Plugins
- [x] `SpellcastPlugin` trait with hooks: on_token_committed, on_token_navigated, on_explain, custom_predictions
- [x] `PluginManager` — register, unload, list, dispatch
- [x] `MedicalDictionaryPlugin` — medical terminology support (built-in)
- [x] `CodeSymbolsPlugin` — code symbol shortcuts (built-in)
- [x] Deliverable: Plugin system (`src/plugin/mod.rs`)
- [ ] **TODO:** Wire PluginManager into terminal event loop
- [ ] **TODO:** Plugin CLI subcommands do not exist (no `spellcast plugins` command)

## TDD Workflow for Each Phase

1. **Write failing test**: Define expected behavior
2. **Run test**: `cargo test` — confirm failure
3. **Write implementation**: Minimum code to pass
4. **Run test**: `cargo test` — confirm pass
5. **Refactor**: Clean up while keeping green
6. **Commit**: `git commit -m "phase-N: feature description"`
7. **Benchmark**: `cargo bench` for perf-sensitive code
