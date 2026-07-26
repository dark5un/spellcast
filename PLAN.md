# VoxKey — Implementation Plan

## Phase 0: Scaffold (Week 1)
- [x] Create project structure (Cargo.toml, directories)
- [ ] Config types (`src/config/`) with TDD
- [ ] Error types (`src/error.rs`) with TDD
- [ ] CLI parsing (`src/main.rs`, `clap`)
- [ ] Default config file (`config/default-config.toml`)
- [ ] Logging setup
- [ ] `setup-bazzite.sh` script
- [ ] `download-models.sh` script
- [ ] `cargo build` passes
- [ ] `cargo clippy` passes
- [ ] `cargo fmt --check` passes
- Deliverable: Working binary that prints help and validates config

## Phase 1: Mode Controller + Config (Week 1-2)
- [ ] Mode enum (`Dictation`, `Raw`, `Killed`)
- [ ] Mode controller tests
- [ ] Kill switch logic (Ctrl+Shift+Escape detection)
- [ ] Caps Lock toggle + Shift+Caps Lock for normal caps
- [ ] TDD: All mode transitions tested
- Deliverable: Mode state machine with tests

## Phase 2: Audio Capture (Week 2-3)
- [ ] Audio capture module with `cpal`
- [ ] Push-to-talk: start/stop recording
- [ ] 16kHz mono 16-bit PCM conversion
- [ ] Audio buffer abstraction
- [ ] TDD: Mock audio device, test buffer pipeline
- Deliverable: Can record audio and produce PCM buffer

## Phase 3: ASR Integration (Week 3-4)
- [ ] ASR trait with `whisper-rs`
- [ ] Model loading (tiny/base models)
- [ ] GPU backend detection (CUDA vs CPU)
- [ ] Audio → text transcription
- [ ] TDD: Mock ASR engine, test integration
- Deliverable: Spoken words → text

## Phase 4: PTY Wrapper (Week 4-5)
- [ ] PTY spawn shell via `portable-pty`
- [ ] Keystroke interception
- [ ] Text injection into PTY
- [ ] Raw passthrough mode (VoxKey transparent)
- [ ] TDD: Mock PTY, test keystroke routing
- Deliverable: Shell runs inside VoxKey, mode switching works

## Phase 5: Tokenizer (Week 5-6)
- [ ] Token types (Prose, CodeIdentifier, Punctuation, etc.)
- [ ] Heuristic tokenizer (regex-based)
- [ ] Context detection (prose vs code)
- [ ] Token navigation (prev/next)
- [ ] Token highlighting in status bar
- [ ] Property-based tests with `proptest`
- Deliverable: Text → token sequence, navigation works

## Phase 6: Phonetic Predictions (Week 6-7)
- [ ] Phonetic encoding via `rphonetic` (Double Metaphone)
- [ ] Prediction engine: top-3 phonetically similar tokens
- [ ] Pre-built phonetic index from common English words
- [ ] Prediction display in status bar
- [ ] Accept prediction (keys 1/2/3)
- [ ] TDD: Phonetic distance tests
- Deliverable: After dictation, 3 predictions shown

## Phase 7: Memory/Learning DB (Week 7-8)
- [ ] SQLite schema (`explained_tokens`, `phonetic_corrections`)
- [ ] DB initialization and migrations
- [ ] Insert/query explained tokens
- [ ] Insert/query phonetic corrections
- [ ] TDD: DB operations tested with in-memory DB
- Deliverable: Persistent memory across sessions

## Phase 8: Explain Feature (Week 8-9)
- [ ] Explain: listen to explanation
- [ ] Local DB lookup by explanation hash
- [ ] LLM fallback via `mistralrs` (or stub for MVP)
- [ ] Web search fallback via `ureq`
- [ ] Result storage in DB
- [ ] TDD: Mock LLM, mock web, test fallback chain
- Deliverable: "Explain" produces correct token

## Phase 9: Integration & Polish (Week 9-10)
- [ ] End-to-end pipeline test
- [ ] Config file loading
- [ ] Latency benchmarks
- [ ] Signal handling (SIGTERM, SIGHUP)
- [ ] Graceful terminal restore on panic
- [ ] Documentation finalization
- Deliverable: Working MVP binary

## Phase 10: Documentation & Packaging (Week 10)
- [ ] README.md
- [ ] ARCHITECTURE.md
- [ ] BAZZITE.md
- [ ] CONFIGURATION.md
- [ ] DEVELOPMENT.md
- [ ] CONTRIBUTING.md
- [ ] LICENSE (Apache 2.0)
- [ ] CHANGELOG.md
- [ ] Final quality gates: build, test, clippy, fmt
- Deliverable: Complete project ready for use

## TDD Workflow for Each Phase

1. **Write failing test**: Define expected behavior
2. **Run test**: `cargo test` — confirm failure
3. **Write implementation**: Minimum code to pass
4. **Run test**: `cargo test` — confirm pass
5. **Refactor**: Clean up while keeping green
6. **Commit**: `git commit -m "phase-N: feature description"`
7. **Benchmark**: `cargo bench` for perf-sensitive code