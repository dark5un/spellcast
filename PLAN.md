# Spellcast — Implementation Plan

## Phase 0: Scaffold (Week 1)
- [x] Create project structure (Cargo.toml, directories)
- [x] Config types (`src/config/`) with TDD
- [x] Error types (`src/error.rs`) with TDD
- [x] CLI parsing (`src/main.rs`, `clap`)
- [x] Default config file (`config/default-config.toml`)
- [x] Logging setup
- [x] `setup-bazzite.sh` script
- [x] `download-models.sh` script
- [x] `cargo build` passes
- [x] `cargo clippy` passes
- [x] `cargo fmt --check` passes
- [x] Deliverable: Working binary that prints help and validates config

## Phase 1: Mode Controller + Config (Week 1-2)
- [x] Mode enum (`Dictation`, `Raw`, `Killed`)
- [x] Mode controller tests
- [x] Kill switch logic (Ctrl+Shift+Escape detection)
- [x] Caps Lock toggle + Shift+Caps Lock for normal caps
- [x] TDD: All mode transitions tested
- [x] Deliverable: Mode state machine with tests

## Phase 2: Audio Capture (Week 2-3)
- [x] Audio capture module with `cpal`
- [x] Push-to-talk: start/stop recording
- [x] 16kHz mono 16-bit PCM conversion
- [x] Audio buffer abstraction
- [x] TDD: Mock audio device, test buffer pipeline
- [x] Deliverable: Can record audio and produce PCM buffer

## Phase 3: ASR Integration (Week 3-4)
- [x] ASR trait with `whisper-rs`
- [x] Model loading (tiny/base models)
- [x] GPU backend detection (CUDA vs CPU)
- [x] Audio → text transcription
- [x] TDD: Mock ASR engine, test integration
- [x] Deliverable: Spoken words → text

## Phase 4: PTY Wrapper (Week 4-5)
- [x] PTY spawn shell via `portable-pty`
- [x] Keystroke interception
- [x] Text injection into PTY
- [x] Raw passthrough mode (Spellcast transparent)
- [x] TDD: Mock PTY, test keystroke routing
- [x] Deliverable: Shell runs inside Spellcast, mode switching works

## Phase 5: Tokenizer (Week 5-6)
- [x] Token types (Prose, CodeIdentifier, Punctuation, etc.)
- [x] Heuristic tokenizer (regex-based)
- [x] Context detection (prose vs code)
- [x] Token navigation (prev/next)
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
- [x] Insert/query explained tokens
- [x] Insert/query phonetic corrections
- [x] TDD: DB operations tested with in-memory DB
- [x] Deliverable: Persistent memory across sessions

## Phase 8: Explain Feature (Week 8-9)
- [x] Explain: listen to explanation
- [x] Local DB lookup by explanation hash
- [x] LLM fallback via `mistralrs` (or stub for MVP)
- [x] Web search fallback via `ureq`
- [x] Result storage in DB
- [x] TDD: Mock LLM, mock web, test fallback chain
- [x] Deliverable: "Explain" produces correct token

## Phase 9: Integration & Polish (Week 9-10)
- [x] End-to-end pipeline test
- [x] Config file loading
- [x] Latency benchmarks
- [x] Signal handling (SIGTERM, SIGHUP)
- [x] Graceful terminal restore on panic
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

## Phase 2K: Packaging (Week 11)
- [x] Fedora RPM spec (`contrib/spellcast.spec`)
- [x] Flatpak manifest (`contrib/io.github.dark5un.spellcast.yml`)
- [x] AppStream metadata (`contrib/io.github.dark5un.spellcast.metainfo.xml`)
- [x] AUR PKGBUILD (`contrib/PKGBUILD`)
- [x] Model management CLI: `spellcast models download`, `spellcast models list`, `spellcast models update`
- [x] Deliverable: Spellcast installable via RPM, Flatpak, or AUR, with offline model management

## Phase 2L: Accessibility (Week 11-12)
- [x] Audio feedback module — beep tones for mode transitions (Dictation/Raw/Killed)
- [x] Screen reader events via `spd-say` for visually impaired users
- [x] Onboarding wizard — interactive 7-step first-run setup
- [x] TDD: Audio feedback tests, screen reader integration tests
- [x] Deliverable: Spellcast accessible to users with visual impairments

## Phase 2M: Plugins (Week 12)
- [x] `SpellcastPlugin` trait with `fn name()`, `fn description()`, `fn on_dictation()`, `fn on_explain()`
- [x] `PluginManager` — load, register, unload, list plugins
- [x] Plugin manager CLI: `spellcast plugins load`, `list`, `unload`
- [x] `MedicalDictionaryPlugin` — medical terminology support (built-in)
- [x] `CodeSymbolsPlugin` — code symbol shortcuts (built-in)
- [x] TDD: Plugin lifecycle tests with mock plugins
- [x] Deliverable: Extensible plugin system with two built-in plugins

## TDD Workflow for Each Phase

1. **Write failing test**: Define expected behavior
2. **Run test**: `cargo test` — confirm failure
3. **Write implementation**: Minimum code to pass
4. **Run test**: `cargo test` — confirm pass
5. **Refactor**: Clean up while keeping green
6. **Commit**: `git commit -m "phase-N: feature description"`
7. **Benchmark**: `cargo bench` for perf-sensitive code