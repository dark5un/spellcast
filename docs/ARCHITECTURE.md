# VoxKey — Architecture Document

> **Version:** 0.1.0  
> **Last Updated:** 2026-07-26  
> **License:** Apache 2.0

## Table of Contents

1. [Overview](#overview)
2. [High-Level Architecture](#high-level-architecture)
3. [Mode State Machine](#mode-state-machine)
4. [Subsystems](#subsystems)
   - [Terminal / PTY Wrapper](#1-terminal--pty-wrapper)
   - [Audio Capture](#2-audio-capture)
   - [ASR Engine](#3-asr-engine)
   - [Tokenizer](#4-tokenizer)
   - [Phonetic Predictor](#5-phonetic-predictor)
   - [Explainer](#6-explainer)
   - [Memory Store](#7-memory-store)
   - [Compute Backend](#8-compute-backend)
   - [Configuration](#9-configuration)
   - [Error Handling](#10-error-handling)
5. [Data Flow](#data-flow)
6. [Deployment Architecture](#deployment-architecture)
7. [Key Design Decisions](#key-design-decisions)
8. [Security Considerations](#security-considerations)
9. [Performance Characteristics](#performance-characteristics)
10. [Future Architecture](#future-architecture)

---

## Overview

VoxKey is a **dictation-first terminal keyboard multiplexer** for Linux. It sits between the user and their shell, intercepting keyboard input and augmenting it with speech-to-text capabilities. The core idea is simple: **when Caps Lock is ON, VoxKey processes speech; when Caps Lock is OFF, VoxKey is transparent.**

### Core Capabilities

| Feature | Description |
|---------|-------------|
| **Dictation** | Speak commands, code, or prose instead of typing |
| **Raw passthrough** | VoxKey is completely transparent (Caps Lock OFF) |
| **Token navigation** | Navigate between *tokens* (not words) with H/L keys |
| **Phonetic predictions** | Up to 3 alternatives ranked by phoneme distance |
| **Explain feature** | Describe a concept verbally, get the right token |
| **Kill switch** | Ctrl+Shift+Escape immediately disables VoxKey |
| **Persistent memory** | Learns from corrections over time via SQLite |
| **Local only** | All processing runs on the user's machine — no cloud |

---

## High-Level Architecture

```
┌──────────────────────────────────────────────────────────────────┐
│                     User Terminal (alacritty/kitty/...)           │
└──────────────────────────┬───────────────────────────────────────┘
                           │ stdin / stdout
                           ▼
┌──────────────────────────────────────────────────────────────────┐
│  ┌────────────────────────────────────────────────────────────┐  │
│  │              VoxKey (Rust Binary)                          │  │
│  │                                                            │  │
│  │  ┌──────────┐  ┌────────┐  ┌──────────┐  ┌────────────┐  │  │
│  │  │  Audio   │──│  ASR   │──│ Tokenizer│  │ Predictor  │  │  │
│  │  │ Capture  │  │ Engine │  │          │  │ (Phonetic) │  │  │
│  │  │ (cpal)   │  │(whisper)│  │          │  │ (rphonetic)│  │  │
│  │  └──────────┘  └────────┘  └─────┬────┘  └────────────┘  │  │
│  │                                  │                        │  │
│  │  ┌──────────┐  ┌──────────┐     │      ┌────────────┐    │  │
│  │  │  Backend │  │  Memory  │     │      │ Explainer  │    │  │
│  │  │  Detect  │  │  (SQLite)│     │      │ DB→LLM→Web │    │  │
│  │  └──────────┘  └──────────┘     │      └────────────┘    │  │
│  │                                 │                        │  │
│  │                    ┌────────────▼──────────┐             │  │
│  │                    │  Mode Controller       │             │  │
│  │                    │  Dictation │ Raw │ Kill│             │  │
│  │                    └────────────┬──────────┘             │  │
│  │                                 │                        │  │
│  │  ┌──────────────────────────────▼────────────────────┐   │  │
│  │  │           PTY Wrapper (portable-pty)              │   │  │
│  │  │  ┌─────────────────────────────────────────────┐  │   │  │
│  │  │  │  Shell (bash/zsh/fish) running in PTY       │  │   │  │
│  │  │  └─────────────────────────────────────────────┘  │   │  │
│  │  └────────────────────────────────────────────────────┘   │  │
│  │                                                            │  │
│  │  ┌────────────────────────────────────────────────────┐   │  │
│  │  │  Status Bar: [DICT] @3:'hello' | 1:world 2:word   │   │  │
│  │  └────────────────────────────────────────────────────┘   │  │
│  └────────────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────────┘
                           │ /dev/uinput (keystroke injection)
                           ▼
                    ┌──────────────┐
                    │  Linux Input │
                    │  Subsystem   │
                    └──────────────┘
```

### Key Architectural Principle

VoxKey is **not** a terminal emulator. It delegates all terminal rendering to the host terminal emulator (alacritty, kitty, gnome-terminal, etc.). VoxKey itself runs as a **command-line program** that spawns a shell in a pseudo-terminal (PTY) and wraps it with a thin status bar overlay.

---

## Mode State Machine

VoxKey operates in three modes, managed by the `ModeController` in `src/modes/mod.rs`:

```
                     ┌─────────────────┐
                     │     RAW         │  (Caps Lock OFF)
                     │  (transparent)  │
                     └────────┬────────┘
                              │ Caps Lock pressed (alone)
                              ▼
                     ┌─────────────────┐
                ┌───▶│   DICTATION     │  (Caps Lock ON)
                │    │  (speech→text)  │
                │    └────────┬────────┘
                │             │ Esc in dictation
                │             ▼ (returns to Raw)
                │    ┌─────────────────┐
                │    │     RAW         │
                │    └─────────────────┘
                │
                │    ┌─────────────────┐
                │    │    KILLED       │  (Ctrl+Shift+Escape)
                └────│  (fully locked) │
                     └─────────────────┘
```

| Transition | Trigger | Effect |
|------------|---------|--------|
| Raw → Dictation | Caps Lock pressed alone | VoxKey activates: speech to text, token nav |
| Dictation → Raw | Caps Lock pressed alone | VoxKey becomes transparent |
| Any → Killed | Ctrl+Shift+Escape | VoxKey fully disabled, all keys passthrough |
| Killed → Raw | Ctrl+Shift+Escape (again) | Returns to raw passthrough |
| Shift+Caps Lock | Any mode | Toggles actual caps lock state (independent of mode) |

**Key design choice:** The mode controller is a synchronous flag — no locks, no channels, no async. Mode transitions are atomic `match` operations, guaranteed to complete in <5μs. This is deliberate: the main event loop polls at 10ms intervals and must never block on mode state.

---

## Subsystems

### 1. Terminal / PTY Wrapper

**Location:** `src/terminal/mod.rs`  
**Key Dependencies:** `portable-pty` (PTY management), `crossterm` (raw mode, events), `ratatui` (status bar rendering)

#### Architecture

```
┌──────────────────────────────────────┐
│  Terminal Emulator (kitty/alacritty) │
│  ┌────────────────────────────────┐  │
│  │ VoxKey Process                 │  │
│  │  ┌─────────────────────────┐   │  │
│  │  │  stdin/stdout           │   │  │
│  │  │  (crossterm raw mode)   │   │  │
│  │  └─────────────────────────┘   │  │
│  │  ┌─────────────────────────┐   │  │
│  │  │  PTY Master             │   │  │
│  │  │  (portable-pty)         │   │  │
│  │  └──────────┬──────────────┘   │  │
│  │             │ PTY Slave        │  │
│  │  ┌──────────▼──────────────┐   │  │
│  │  │  Shell (bash)           │   │  │
│  │  └─────────────────────────┘   │  │
│  │  ┌─────────────────────────┐   │  │
│  │  │  Status Bar (ratatui)   │   │  │
│  │  │  [DICT] @3:'hello'      │   │  │
│  │  └─────────────────────────┘   │  │
│  └────────────────────────────────┘  │
└──────────────────────────────────────┘
```

#### Event Loop

The terminal module runs the main event loop (`run_terminal_loop`):

```
loop {
    1. Render status bar (cursor-save, position, cursor-restore)
    2. Check shell liveness (try_wait)
    3. Read PTY output → write to stdout
    4. Poll keyboard events (10ms timeout)
    5. If key event: route through mode controller
    6. Handle resize events → resize PTY
}
```

**Important implementation details:**

- The status bar is rendered via ANSI escape sequences, not ratatui's frame system. VoxKey saves the cursor position, jumps to the bottom row, writes the status bar in reverse video, and restores the cursor. This avoids the complexity of a full TUI application while keeping the shell's scrollback intact.
- The `TerminalGuard` struct uses Rust's `Drop` trait to guarantee terminal restoration on panic, ensuring the user is never left with a broken terminal.
- Signal handlers (`SIGTERM`, `SIGINT`) via the `ctrlc` crate provide an additional safety net.
- Keystrokes are written to the PTY as raw byte sequences (e.g., `\x1b[D` for left arrow), not character-by-character.

#### PTY Lifecycle

1. `native_pty_system()` → creates a platform PTY system
2. `openpty(size)` → creates a master/slave PTY pair
3. `spawn_command(shell)` → spawns the shell connected to the slave
4. `master.try_clone_reader()` → get reader for shell output
5. `master.take_writer()` → get writer for keystroke injection
6. On exit: `child.try_wait()` → detect shell exit → clean up

---

### 2. Audio Capture

**Location:** `src/audio/mod.rs`  
**Key Dependencies:** `cpal` (cross-platform audio I/O)

#### Pipeline

```
Microphone → cpal input stream → i16 PCM samples → resample to 16kHz
                                                         │
                                                         ▼
                                              AudioBuffer { samples, sample_rate }
                                                         │
                                                         ▼
                                              AudioBuffer::to_f32() → f32 samples
                                                         │
                                                         ▼
                                                      ASR Engine
```

#### Design

- `AudioCapture::new(config)` → opens the microphone device via `cpal`
- `record_duration(secs)` → records for a fixed duration (MVP approach)
- `record_push_to_talk(secs)` → same as `record_duration` for MVP; future versions will use VAD or key-toggle
- Raw PCM samples are collected in an `Arc<Mutex<Vec<i16>>>` during recording
- Samples are resampled to 16kHz mono (Whisper's expected input format) using linear interpolation
- `AudioBuffer::to_f32()` converts i16 to f32 normalized to [-1.0, 1.0]

**Mock for testing:** `MockAudioCapture` generates synthetic 440 Hz sine wave buffers, allowing ASR tests without hardware.

**Known limitation:** Push-to-talk is currently a fixed-duration recording. The architecture supports a toggle-based start/stop pattern, but the MVP doesn't implement VAD (voice activity detection).

---

### 3. ASR Engine

**Location:** `src/asr/mod.rs`  
**Key Dependencies:** `whisper-rs` (whisper.cpp bindings)

#### Trait Design

```rust
pub trait AsrEngine: Send {
    fn load_model(&mut self, model_path: &str) -> VoxKeyResult<()>;
    fn transcribe(&self, audio: &AudioBuffer) -> VoxKeyResult<AsrResult>;
    fn is_ready(&self) -> bool;
}
```

#### Implementations

| Implementation | Feature Flag | Use Case |
|---------------|--------------|----------|
| `WhisperAsr` | `whisper-rs` | Production ASR via whisper.cpp |
| `NoopAsr` | Always | Testing without model files |

#### `WhisperAsr` Pipeline

```
AudioBuffer → to_f32() → f32 samples
                              │
                              ▼
whisper_rs::WhisperContext → create_state()
                              │
                              ▼
FullParams { Greedy { best_of: 5 }, language: "en" }
                              │
                              ▼
state.full(params, &audio_f32)
                              │
                              ▼
state.as_iter() → collect segments → String
```

**Key parameters:**
- `SamplingStrategy::Greedy { best_of: 5 }` for high-quality transcription
- `language: "en"` (configurable)
- `no_timestamps: true` (we only need the text, not word-level timestamps for MVP)

#### Model Loading

- Models are loaded from disk (downloaded via `scripts/download-models.sh`)
- Feature-gated compute backends: `cuda`, `vulkan`, `cpu`
- The `WhisperContextParameters` default is used; future versions may expose context size and thread count

---

### 4. Tokenizer

**Location:** `src/tokenizer/mod.rs`  
**Key Dependencies:** `regex` (pattern matching)

#### Token Types

```rust
pub enum TokenType {
    Word,              // "hello", "don't"
    CodeIdentifier,    // "fooBar", "my_variable"
    Punctuation,       // ".", ",", "!"
    Operator,          // "->", "=>", "::"
    Whitespace,        // " ", "\t", "\n"
    Number,            // "42", "3.14"
    StringLiteral,     // '"hello"' (future)
    Other,             // Fallback
}
```

#### Context Detection

The tokenizer heuristically detects whether the text is **prose** or **code**:

1. **Code indicators** — checks for keywords like `fn`, `let`, `struct`, `impl`, `->`, `=>`, `::`
2. **Identifier patterns** — counts camelCase and snake_case patterns
3. **Threshold** — if 2+ code identifiers are found, classify as Code context

#### TokenStream

```rust
pub struct TokenStream {
    pub tokens: Vec<Token>,
    pub context: TokenContext,  // Prose | Code
}
```

The `TokenStream` supports navigation operations: `get()`, `remove()`, `insert()`, `replace()` — enabling in-place editing of dictated text without round-tripping through the shell.

**Design decision:** The tokenizer uses simple heuristic regex patterns rather than a parser generator or tree-sitter. This keeps startup time under 1ms and the binary size minimal. For the MVP, this is sufficient for the common case of terminal dictation (commands, short prose, code snippets). A tree-sitter integration is a planned improvement.

---

### 5. Phonetic Predictor

**Location:** `src/predictor/mod.rs`  
**Key Dependencies:** `rphonetic` (Double Metaphone), custom Levenshtein distance

#### Algorithm

```
1. User dictates a word → ASR transcribes it
2. Encode the transcribed word using Double Metaphone → phonetic code
3. Compare against a pre-built phonetic index (HashMap<String, String>)
4. Compute Levenshtein edit distance on phonetic codes
5. Return top-3 closest matches (excluding the input word itself)
```

#### Index

The `Predictor` maintains an in-memory `HashMap<word, phonetic_code>`:

- **Build phase:** Words are encoded via Double Metaphone and stored
- **Query phase:** The input word is encoded, then the index is scanned for phonetic neighbors
- **Default vocabulary:** ~130 common English words + programming keywords (`function`, `struct`, `impl`, `mut`, etc.)
- **Extensible:** `add_word()` allows incremental index growth from user corrections

#### Performance

- Index build: O(n) where n is vocabulary size
- Query: O(n) scan + O(n log n) sort for top-k — acceptable for n < 10,000
- Levenshtein distance: O(|a| × |b|) per comparison on short phonetic codes
- Expected query time: <10ms for a 10,000-word index

**Design note:** The MVP uses a linear scan. For larger vocabularies, a BK-tree or VP-tree index would reduce query complexity to O(log n).

---

### 6. Explainer

**Location:** `src/explainer/mod.rs`  
**Key Dependencies:** `sha2` (hashing), `mistralrs` (optional LLM), `ureq` (web search)

#### Fallback Chain

The explainer implements a three-tier fallback chain:

```
User says: "a collection of items"
              │
              ▼
     ┌────────────────┐
     │ Step 1: DB     │ ← Hash the explanation, look up in SQLite
     │ Cache Hit?     │
     └────────┬───────┘
         Yes  │     No
         ◄────┤
              ▼
     ┌────────────────┐
     │ Step 2: LLM    │ ← Query local LLM (mistralrs, optional)
     │ Available?     │
     └────────┬───────┘
         Yes  │     No (or fails)
         ◄────┤
              ▼
     ┌────────────────┐
     │ Step 3: Web    │ ← Dictionary lookup (MVP stub)
     │ Search         │     Future: DuckDuckGo API
     └────────┬───────┘
         Yes  │     No
         ◄────┤
              ▼
     ┌────────────────┐
     │ Fallback: raw  │ ← Return the explanation text as-is
     │ explanation     │
     └────────────────┘
              │
              ▼
     Store in DB for future cache hits
```

#### Dictionary (MVP Stub)

For the MVP, the "web search" fallback is a hardcoded `Dictionary` of ~20 concept-token pairs:
- `"a collection of items"` → `"array"`
- `"iterate over a collection"` → `"for loop"`
- `"a named block of code"` → `"function"`
- `"a key value store"` → `"hash map"`
- etc.

This provides a functional demo without requiring a local LLM or network access.

#### LLM Integration (Optional)

When the `llm` feature flag is enabled, the explainer uses `mistralrs` to query a local model:
- Default model: `Qwen/Qwen3-4B` (small, fast, runs on consumer GPUs)
- Auto-quantization to 4-bit (ISQ) via `mistralrs`
- Tokio runtime created on-demand for the blocking call

---

### 7. Memory Store

**Location:** `src/memory/mod.rs`  
**Key Dependencies:** `rusqlite` (SQLite), `sha2` (hashing)

#### Schema

```sql
-- Explained tokens: caches concept-to-token mappings
CREATE TABLE explained_tokens (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    language_context TEXT NOT NULL,
    explanation_hash TEXT NOT NULL UNIQUE,
    explanation_text TEXT NOT NULL,
    token           TEXT NOT NULL,
    created_at      INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    usage_count     INTEGER NOT NULL DEFAULT 1,
    last_used       INTEGER NOT NULL DEFAULT (strftime('%s', 'now'))
);
CREATE INDEX idx_explained_hash ON explained_tokens(explanation_hash);
CREATE INDEX idx_explained_context ON explained_tokens(language_context);

-- Phonetic corrections: learns from user corrections over time
CREATE TABLE phonetic_corrections (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    spoken_text     TEXT NOT NULL,
    corrected_token TEXT NOT NULL,
    language_context TEXT NOT NULL,
    created_at      INTEGER NOT NULL DEFAULT (strftime('%s', 'now')),
    usage_count     INTEGER NOT NULL DEFAULT 1
);
CREATE INDEX idx_phonetic_spoken ON phonetic_corrections(spoken_text);
CREATE INDEX idx_phonetic_context ON phonetic_corrections(language_context);
```

#### Key design choices

- **Hashed keys:** Explanations are SHA-256 hashed (truncated to 8 bytes, 16 hex chars) for lookup. This avoids storing long explanation text as primary keys and enables constant-time lookup.
- **Usage tracking:** Both tables track `usage_count`. Frequently-used mappings get promoted in search results. The `explained_tokens` table also tracks `last_used` for LRU-style cache eviction (future).
- **Bundled SQLite:** The `bundled` feature of `rusqlite` ships SQLite with the binary, avoiding system dependency issues.
- **In-memory mode:** `MemoryStore::open_in_memory()` enables fast, isolated tests without filesystem I/O.

---

### 8. Compute Backend

**Location:** `src/backend/mod.rs`

#### Detection Order

```
Auto-detect:
  1. Try nvidia-smi → parse GPU name, compute capability, driver version
  2. If CUDA GPU found → ComputeBackend { type: Cuda, description: "CUDA (RTX 5090, SM 10.0)" }
  3. If no CUDA → log warning → return CPU backend
```

#### Backend Configuration

```rust
pub enum BackendType {
    Auto,    // Auto-detect: CUDA → Vulkan → CPU
    Cuda,    // NVIDIA CUDA
    Vulkan,  // Not yet implemented in MVP
    Cpu,     // CPU-only fallback
}
```

The backend detection is **informational** at this stage — it identifies which GPU is available but the actual acceleration is handled by the respective libraries (`whisper-rs` for ASR, `mistralrs` for LLM) which have their own CUDA backend selection. The `ComputeBackend` descriptor is available for:
- Logging and diagnostics
- Future selection of model size based on VRAM
- Fallback decisions when GPU memory is insufficient

---

### 9. Configuration

**Location:** `src/config/mod.rs`  
**Format:** TOML, loaded from `~/.config/voxkey/config.toml` (XDG-compliant)

#### Configuration Sections

```toml
[backend]
type = "auto"                    # auto | cuda | vulkan | cpu

[audio]
sample_rate = 16000              # Whisper expects 16kHz
channels = 1                     # Mono
device = "default"               # Microphone device name

[asr]
engine = "whisper-cpp"           # ASR engine backend
model_path = "~/.config/voxkey/models/ggml-base.en.bin"
language = "en"

[llm]
engine = "mistral-rs"            # LLM for explain feature
model_path = "Qwen/Qwen3-4B"     # HuggingFace model ID
max_tokens = 50

[keys]
mode_toggle = "CapsLock"
caps_toggle = "Shift+CapsLock"
prev_token = "h"
next_token = "l"
redictate = "r"
delete_token = "x"
explain = "e"
kill_switch = "Ctrl+Shift+Escape"

[tokenizer]
mode = "heuristic"               # heuristic | tree-sitter (future)
default_context = "prose"        # prose | code

[database]
path = "~/.config/voxkey/voxkey.db"

[languages]
primary = "en"
secondary = "none"
```

#### CLI Arguments (`clap`)

| Flag | Description | Default |
|------|-------------|---------|
| `-c, --config` | Config file path | `~/.config/voxkey/config.toml` |
| `-b, --backend` | Backend override | From config |
| `-s, --shell` | Shell to spawn | `$SHELL` or `/bin/bash` |
| `-v, --verbose` | Verbose logging | false |

**Important:** If the config file doesn't exist, VoxKey falls back to defaults without error. If the file exists but is malformed, VoxKey returns a parse error and exits. This "missing = OK, malformed = failure" behavior is deliberate: first-time users get a working default without any setup.

---

### 10. Error Handling

**Location:** `src/error.rs`  
**Crate:** `thiserror`

#### Error Taxonomy

```rust
pub enum VoxKeyError {
    // Configuration
    Config(String),           // File not found or unreadable
    ConfigParse(toml::de::Error),  // Malformed TOML

    // Audio
    Audio(String),            // Device not found, stream failed

    // ASR
    AsrModel(String),         // Model loading failure
    AsrInference(String),     // Transcription failure

    // Tokenizer
    Tokenizer(String),        // Tokenization failure

    // Predictor
    Predictor(String),        // Phonetic prediction failure

    // Explainer
    ExplainerDb(String),      // Cache lookup failure
    Llm(String),              // LLM inference failure
    WebSearch(String),        // Web search failure

    // Database
    Database(rusqlite::Error),

    // Terminal
    TerminalPty(String),      // PTY creation/management
    TerminalRender(String),   // Status bar rendering

    // Backend
    Backend(String),          // Compute backend init

    // I/O
    Io(std::io::Error),

    // General
    Internal(String),
}
```

**Design philosophy:** Errors are grouped by subsystem (Audio, Asr, Terminal, etc.) rather than by severity. This makes it easy to add subsystem-specific recovery logic later (e.g., retry audio stream on `Audio` error, fall back to CPU on `Backend` error).

---

## Data Flow

### Dictation Flow (simplified)

```
1. Caps Lock ON
       │
2. User speaks: "hello world"
       │
3. AudioCapture::record_push_to_talk(3.0)
       │
4. AudioBuffer { samples: [...], sample_rate: 16000 }
       │
5. WhisperAsr::transcribe(&buffer)
       │
6. AsrResult { text: "hello world", inference_ms: 250 }
       │
7. HeuristicTokenizer::tokenize("hello world")
       │
8. TokenStream { tokens: [hello, " ", world], context: Prose }
       │
9. Inject token text into PTY
       │
10. Render status bar: [DICT] hello world
       │
11. Predictor::predict("hello", 3)  → ["helm", "held", "help"]
       │
12. Status bar: [DICT] @0:'hello' | 1:helm 2:held 3:help
```

### Explain Flow

```
1. User navigates to a token (H/L keys)
2. Presses 'E' → explain triggered
3. User says: "a collection of items"
4. AudioCapture → ASR → "a collection of items"
5. Explainer::explain("a collection of items", "prose")
       │
   ├── 5a. DB lookup by hash → miss
   ├── 5b. LLM fallback (if enabled) → "array"
   └── 5c. Dictionary lookup → "array"
       │
6. Store in DB: hash("a collection of items") → "array"
7. Replace current token with "array"
8. Render status bar
```

### Kill Switch Flow

```
1. User presses Ctrl+Shift+Escape
2. ModeController::toggle_kill_switch() → Mode::Killed
3. KILL_SWITCH_ENGAGED = true (AtomicBool)
4. All subsequent key events pass through to PTY unchanged
5. Status bar: [KILLED]
6. Press Ctrl+Shift+Escape again → Mode::Raw → VoxKey re-enabled
```

---

## Deployment Architecture

### Container Strategy

VoxKey uses a **distrobox-first** deployment model:

```
┌─────────────────────────────────────────────────────────────┐
│ Host (Bazzite / Fedora Silverblue)                          │
│  ┌───────────────────────────────────────────────────────┐  │
│  │ distrobox container "voxkey-dev"                      │  │
│  │  ┌─────────────────────────────────────────────────┐  │  │
│  │  │  VoxKey binary                                   │  │  │
│  │  │  - Whisper model files (~/.config/voxkey/models/)│  │  │
│  │  │  - SQLite DB (~/.config/voxkey/voxkey.db)        │  │  │
│  │  │  - Config (~/.config/voxkey/config.toml)         │  │  │
│  │  └─────────────────────────────────────────────────┘  │  │
│  │                                                       │  │
│  │  Bind mounts (automatic via distrobox):               │  │
│  │  - /dev/uinput        → keystroke injection           │  │
│  │  - /run/user/*/pipewire-0 → microphone access          │  │
│  │  - NVIDIA devices     → GPU acceleration               │  │
│  │  - $HOME              → config, models, DB             │  │
│  └───────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

#### Why distrobox?

| Need | Solution |
|------|----------|
| GPU passthrough | `distrobox create --nvidia` |
| Audio passthrough | Automatic PipeWire socket mount |
| uinput access | `/dev` passthrough (default) |
| User home access | Automatic `$HOME` bind mount |
| Isolation from host | Fedora container on any host |

#### Alternative: Podman Standalone

The `Containerfile` also supports standalone podman usage, but audio and GPU passthrough require explicit flags (documented in the Containerfile). Distrobox is the recommended path.

### Filesystem Layout

```
~/.config/voxkey/
├── config.toml          # User configuration (auto-generated on first run)
├── voxkey.db            # SQLite memory store
└── models/
    ├── ggml-base.en.bin   # Whisper base model (English)
    └── ggml-tiny.en.bin   # Whisper tiny model (lighter, faster)
```

---

## Key Design Decisions

### 1. PTY Wrapper vs Terminal Emulator

**Decision:** VoxKey uses `portable-pty` to spawn a shell inside a PTY, rather than building a full terminal emulator.

**Rationale:** Building a terminal emulator would add enormous complexity (rendering, scrollback, fonts, Sixel, etc.) and duplicate the work of mature terminal emulators. By wrapping a PTY, VoxKey delegates all rendering to the host terminal while maintaining full control over keystroke interception and injection.

**Trade-off:** VoxKey cannot render inline content (e.g., colored tokens directly in the terminal body). The status bar is the only UI element, rendered via ANSI escape sequences.

### 2. Heuristic Tokenizer vs Tree-Sitter

**Decision:** The MVP uses a regex-based heuristic tokenizer.

**Rationale:** Tree-sitter requires loading grammar files, compiling them, and maintaining per-language queries. For a dictation-first tool where most input is either prose or short command lines, a heuristic tokenizer is sufficient. Tree-sitter integration is planned for Phase 11+ when we need language-aware code navigation.

### 3. Linear Scan Phonetic Index vs BK-Tree

**Decision:** The MVP uses a linear scan (O(n)) of the phonetic index.

**Rationale:** With a vocabulary of <10,000 words, a linear scan completes in under 10ms — well within the 100ms UX budget. A BK-tree or VP-tree would add complexity and maintenance burden without meaningful benefit at this scale.

### 4. Blocking Audio Capture vs Async

**Decision:** Audio capture blocks the current thread via `std::thread::sleep`.

**Rationale:** The main event loop runs at 10ms poll intervals. Audio capture happens on-demand (triggered by user action) and the blocking pattern is simpler than async audio stream management. The `record_duration` approach is a deliberate MVP simplification — a future version will use non-blocking VAD-based capture.

### 5. In-Process LLM vs External API

**Decision:** LLM inference is in-process via `mistralrs` behind a feature flag.

**Rationale:** VoxKey's design principle is "local only." Making an external API call would break this principle. `mistralrs` provides Rust-native LLM inference that loads directly into the VoxKey process. However, LLM inference is `optional` (behind the `llm` feature flag) to avoid bloating the binary for users who only need ASR.

### 6. SQLite vs Custom File Format

**Decision:** SQLite via `rusqlite` with bundled feature.

**Rationale:** SQLite provides ACID transactions, indexed lookups, and schema management without introducing any external dependency. The `bundled` feature compiles SQLite into the binary (no system SQLite required). The schema is simple enough (2 tables + indexes) that no ORM is needed.

### 7. Modes as Atomic State

**Decision:** Mode transitions are synchronous, lock-free operations on a simple `enum` field.

**Rationale:** The mode controller is accessed from a single thread (the main event loop). Using locks or channels would add unnecessary complexity. The kill switch uses an `AtomicBool` (accessed from signal handlers) in addition to the mode controller for signal-safe access.

---

## Security Considerations

### Threat Model

VoxKey operates as a **keystroke interceptor and microphone listener**. Its security posture must account for:

| Threat | Mitigation |
|--------|------------|
| Keystroke injection by malicious processes | VoxKey requires `/dev/uinput` access (root or `input` group). The udev rule `99-voxkey-uinput.rules` restricts this to the `input` group. |
| Microphone accessed without consent | VoxKey only captures audio when in Dictation mode AND when the user explicitly triggers push-to-talk. Audio capture is never automatic. |
| Model file tampering | Model files are read-only after download; VoxKey does not execute code from model files (whisper.cpp is sandboxed in a C FFI). |
| SQLite injection | All user input is parameterized via `rusqlite::params!()`. No raw string concatenation. |
| Config file injection | TOML deserialization is schema-bound via `serde`. Unknown fields are ignored. |
| Panic in signal handler | The signal handler uses `AtomicBool` (lock-free) and is minimal. Terminal restoration is handled by `Drop`. |

### Attack Surface

1. **Audio input** — VoxKey opens the microphone device. A malicious user on the same machine could pipe audio to the device. Mitigation: standard Linux audio permissions (PipeWire socket).
2. **PTY I/O** — VoxKey reads and writes to a PTY. The shell inside the PTY has the same security properties as any shell session (user-level, no privilege escalation).
3. **Model files** — Large binary files (~75MB for base.en, ~1.5GB for large-v3) loaded via FFI. Whisper.cpp has undergone extensive fuzzing.
4. **LLM inference** — Optional feature. The local model runs entirely in-process. No data leaves the machine.

### Recommended Host Setup

```bash
# Add user to input group (for /dev/uinput)
sudo usermod -a -G input $USER

# Install udev rule (run once)
sudo cp contrib/99-voxkey-uinput.rules /etc/udev/rules.d/
sudo udevadm control --reload-rules && sudo udevadm trigger

# Re-login or newgrp to apply group changes
```

---

## Performance Characteristics

| Operation | Expected Latency | Notes |
|-----------|-----------------|-------|
| Keystroke passthrough | <1ms | Crossterm raw mode → PTY write |
| Mode switch | <5μs | Single atomic flag toggle |
| Tokenization (heuristic) | <0.5ms | Regex-based on short text |
| Phonetic prediction (3 candidates, 10k vocab) | <10ms | Linear scan + sort |
| ASR (GPU, tiny model, 3s audio) | 100-300ms | whisper.cpp CUDA |
| ASR (GPU, base model, 3s audio) | 200-500ms | whisper.cpp CUDA |
| ASR (CPU, tiny model, 3s audio) | 500-3000ms | Depends on CPU |
| Explain (DB cache hit) | <2ms | SQLite lookup by hash |
| Explain (LLM, GPU, 4B model) | 500-2000ms | Qwen/Qwen3-4B, 4-bit quantized |
| Explain (dictionary fallback) | <1ms | HashMap lookup |
| PTY output read (buffer) | <1ms | Non-blocking read |
| Status bar render | <1ms | ANSI escape sequence |
| Startup time | 200-500ms | Config load, ASR model load |
| Memory usage (idle) | ~150MB | With base.en model loaded |
| Memory usage (dictating) | ~300MB | With base.en + Qwen3-4B 4-bit |
| Binary size (release, no LLM) | ~8MB | Stripped |
| Binary size (release, with LLM) | ~35MB | Stripped, includes mistralrs |

### Optimization Headroom

1. **ASR latency:** Switch from base model (244M params) to tiny model (39M params) for ~3x speedup at the cost of accuracy.
2. **LLM latency:** Use a smaller model (Qwen2.5-1.5B, Phi-3.5-mini) or disable LLM entirely.
3. **Phonetic search:** Replace linear scan with BK-tree for O(log n) lookup with larger vocabularies.
4. **Audio capture:** Implement VAD (voice activity detection) to avoid fixed-duration recording and reduce unnecessary ASR calls.
5. **Tokenizer:** Pre-compile regexes into `LazyLock` statics to avoid recompilation.

---

## Future Architecture

### Phase 11+: Planned Improvements

| Feature | Description | Impact |
|---------|-------------|--------|
| Tree-sitter tokenizer | Per-language AST-aware tokenization | Better code navigation |
| VAD (Voice Activity Detection) | Non-blocking push-to-talk | Faster, more natural dictation |
| Streaming ASR | Real-time transcription via sherpa-onnx | Lower latency |
| BK-tree phonetic index | O(log n) phonetic lookup | Support for 100k+ vocabularies |
| Multi-language support | French, Spanish, German ASR | Broader audience |
| Plugin system | User-extensible commands | Ecosystem growth |
| GUI configurator | Visual config editor | Better UX |
| Phonetic index editor | Add/remove words from the index | User customization |

### Scaling Considerations

- **Vocabulary size:** The phonetic index is in-memory. At 100k words (covering English + common programming terms), memory usage would be ~20MB. Beyond this, consider mmap-backed or SQLite-backed phonetic indexes.
- **Concurrent sessions:** VoxKey is a single-user, single-session tool. Multiple concurrent sessions (e.g., tmux panes) would require separate VoxKey instances.
- **Model size:** Whisper large-v3 (~3GB VRAM) runs on any modern GPU with 4GB+ VRAM. The default base.en model (~1GB VRAM) runs on virtually any GPU, including integrated graphics via Vulkan.