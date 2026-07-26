# Spellcast — Architecture Document

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
   - [Voice Activity Detection](#3-voice-activity-detection-vad)
   - [ASR Engine](#4-asr-engine)
   - [Tokenizer](#5-tokenizer)
   - [Phonetic Predictor](#6-phonetic-predictor)
   - [Explainer](#7-explainer)
   - [Memory Store](#8-memory-store)
   - [Compute Backend](#9-compute-backend)
   - [Configuration](#10-configuration)
   - [Error Handling](#11-error-handling)
   - [Plugin System](#12-plugin-system)
   - [Accessibility / Onboarding](#13-accessibility--onboarding)
5. [Data Flow](#data-flow)
6. [Deployment Architecture](#deployment-architecture)
7. [Key Design Decisions](#key-design-decisions)
8. [Security Considerations](#security-considerations)
9. [Performance Characteristics](#performance-characteristics)
10. [Future Architecture](#future-architecture)

---

## Overview

Spellcast is a **dictation-first terminal keyboard multiplexer** for Linux. It sits between the user and their shell, intercepting keyboard input and augmenting it with speech-to-text capabilities. The core idea: **in Dictation mode, Spellcast captures audio continuously via VAD, transcribes speech to text, and injects it into the PTY; in Raw mode, Spellcast is transparent.**

### Core Capabilities

| Feature | Description |
|---------|-------------|
| **Dictation** | Speak commands, code, or prose instead of typing — VAD-based continuous listening |
| **Raw passthrough** | Spellcast is completely transparent |
| **Token navigation** | Navigate between *tokens* (not words) with `h`/`l` keys |
| **Token editing** | `x` deletes, `r` re-dictates the highlighted token |
| **Phonetic predictions** | Up to 3 alternatives ranked by phoneme distance; accept with `1`/`2`/`3` |
| **Explain feature** | `e` on a token triggers concept-to-word lookup (DB → LLM → web search) |
| **Kill switch** | Ctrl+G immediately disables Spellcast; Ctrl+G again re-enables it |
| **Persistent memory** | Learns from corrections over time via SQLite |
| **Local only** | All processing runs on the user's machine — no cloud |

---

## High-Level Architecture

### Layered View

- **User Terminal** (alacritty/kitty/gnome-terminal) — stdin/stdout
  - **Spellcast (Rust Binary)**
    - Pipeline: Audio Capture (cpal) -> VAD (silero) -> ASR Engine (whisper-rs) -> Tokenizer -> Predictor (rphonetic)
    - Supporting modules:
      - Backend Detect — auto-detects CUDA/CPU
      - Memory (SQLite) — persistent corrections and explain cache
      - Explainer — DB -> LLM -> Web search fallback chain
    - Mode Controller — Dictation | Raw | Kill (Ctrl+Space toggle, Ctrl+G kill switch)
    - **PTY Wrapper (portable-pty)** — spawns shell (bash/zsh/fish) inside a pseudo-terminal
    - Status Bar (raw ANSI): `[DICT] @3:'hello' | 1:world`

### Key Architectural Principle

Spellcast is **not** a terminal emulator. It delegates all terminal rendering to the host terminal emulator (alacritty, kitty, gnome-terminal, etc.). Spellcast itself runs as a **command-line program** that spawns a shell in a pseudo-terminal (PTY) and wraps it with a thin status bar overlay rendered via raw ANSI escape sequences (not a TUI framework).

---

## Mode State Machine

Spellcast operates in three modes, managed by the `ModeController` in `src/modes/mod.rs`:

Mode transitions (managed by `ModeController` in `src/modes/mod.rs`):

- **Raw** (default) — transparent passthrough
  - Ctrl+Space (or Caps Lock) -> **Dictation** (speech-to-text, VAD listening)
  - Ctrl+Space again returns to Raw
- **Killed** (Ctrl+G) — fully locked, all keys passthrough
  - Ctrl+G again returns to Raw
- Shift+Caps Lock always toggles the actual caps lock state (independent of mode)

| Transition | Trigger | Effect |
|------------|---------|--------|
| Raw → Dictation | Ctrl+Space (or Caps Lock) | Spellcast activates: speech to text, token nav |
| Dictation → Raw | Ctrl+Space (or Caps Lock) | Spellcast becomes transparent |
| Any → Killed | Ctrl+G | Spellcast fully disabled, all keys passthrough |
| Killed → Raw | Ctrl+G (again) | Returns to raw passthrough |
| Shift+Caps Lock | Any mode | Toggles actual caps lock state (independent of mode) |

**Key design choice:** The mode controller is a synchronous flag — no locks, no channels, no async. Mode transitions are atomic `match` operations, guaranteed to complete in <5μs. This is deliberate: the main event loop polls at 50ms intervals and must never block on mode state.

**Note on Caps Lock:** Ctrl+Space is the universal mode toggle. Caps Lock also works as a toggle if the terminal emulator supports the kitty keyboard protocol (which sends Caps Lock as a distinct key event). Shift+Caps Lock always toggles the actual caps lock state.

---

## Subsystems

### 1. Terminal / PTY Wrapper

**Location:** `src/terminal/mod.rs`  
**Key Dependencies:** `portable-pty` (PTY management), `crossterm` (raw mode, events)

#### Architecture

#### Architecture

- **Terminal Emulator** (kitty/alacritty) hosts the Spellcast process
  - **stdin/stdout** (crossterm raw mode)
  - **PTY Master** (portable-pty)
    - PTY Slave -> Shell (bash)
  - **Status Bar** (raw ANSI): `[DICT] @3:'hello'`

#### Event Loop

The terminal module runs the main event loop (`run_terminal_loop` → `run_inner`):

```
loop {
    1. Check shell liveness (try_wait)
    2. Drain PTY output from mpsc channel → write to stdout (non-blocking)
    3. Check for ASR result from mpsc channel (non-blocking)
    4. Render status bar (ANSI escape: save cursor, bottom row, reverse video, restore)
    5. Poll keyboard events (50ms timeout)
    6. If key event: route through mode controller
    7. Handle resize events → resize PTY
}
```

**Important implementation details:**

- **PTY reader on background thread:** The PTY reader runs on a dedicated background thread, sending output via an `mpsc::channel<Vec<u8>>`. The main event loop drains the channel non-blocking (`try_recv`), so PTY output never blocks the event loop.
- **ASR on background thread:** Audio recording and ASR transcription happen on a background thread. Results are sent back via an `mpsc::channel<Result<String, String>>`. The main loop checks `asr_busy` and drains the channel non-blocking.
- **Status bar via raw ANSI escapes:** The status bar is rendered via ANSI escape sequences, not a TUI framework. Spellcast saves the cursor position (`\x1b[s`), jumps to the bottom row, writes the status bar in reverse video (`\x1b[7m...\x1b[0m`), and restores the cursor (`\x1b[u`). This avoids the complexity of a full TUI application while keeping the shell's scrollback intact.
- **Alternate screen:** Spellcast enters the alternate screen (`EnterAlternateScreen`) on startup and leaves on exit. Raw mode is enabled before entering the alternate screen.
- The `TerminalGuard` struct uses Rust's `Drop` trait to guarantee terminal restoration on panic, ensuring the user is never left with a broken terminal.
- Signal handlers (`SIGTERM`, `SIGINT`) via the `ctrlc` crate provide an additional safety net.
- Keystrokes are written to the PTY as raw byte sequences (e.g., `\x1b[D` for left arrow), not character-by-character.

#### PTY Lifecycle

1. `native_pty_system()` → creates a platform PTY system
2. `openpty(size)` → creates a master/slave PTY pair
3. `spawn_command(shell)` → spawns the shell connected to the slave
4. `master.try_clone_reader()` → get reader for shell output (moved to background thread)
5. `master.take_writer()` → get writer for keystroke injection
6. On exit: `child.try_wait()` → detect shell exit → clean up

---

### 2. Audio Capture

**Location:** `src/audio/mod.rs`  
**Key Dependencies:** `cpal` (cross-platform audio I/O, PipeWire backend)

#### Pipeline

```
Microphone -> cpal input stream (PipeWire) -> f32 samples
    |
    |-- record_duration(secs) -> AudioBuffer { samples, sample_rate: 16kHz }
    |                              -> AudioBuffer::to_f32() -> f32 samples -> ASR Engine
    |
    `-- start_continuous(chunk_tx) -> chunks via mpsc::Sender<Vec<f32>> (for VAD streaming)
```

#### Design

- `AudioCapture::new(config)` → opens the microphone device via `cpal` (PipeWire backend)
- `record_duration(secs)` → records for a fixed duration (blocks the current thread). Used by the push-to-talk Space key.
- `start_continuous(chunk_tx)` → starts a continuous audio stream, sending f32 chunks via an `mpsc::Sender<Vec<f32>>`. Used for VAD-based continuous capture.
- `record_push_to_talk(secs)` → currently same as `record_duration` (MVP).
- Raw PCM samples are collected during recording
- Samples are resampled to 16kHz mono (Whisper's expected input format) using linear interpolation
- `AudioBuffer::to_f32()` converts i16 to f32 normalized to [-1.0, 1.0]

**Mock for testing:** `MockAudioCapture` generates synthetic 440 Hz sine wave buffers, allowing ASR tests without hardware.

**Note on dictation mode:** The terminal event loop currently uses `record_duration(3.0)` triggered by the Space key (push-to-talk). The VAD module (`src/audio/vad.rs`) and `start_continuous` exist for continuous VAD-based capture but are not yet wired into the terminal event loop.

---

### 3. Voice Activity Detection (VAD)

**Location:** `src/audio/vad.rs`  
**Key Dependencies:** `silero` crate v0.6.0 (Silero VAD ONNX model, bundled)

#### Components

| Component | Description |
|-----------|-------------|
| `VoiceActivityDetector` | Wraps the Silero VAD ONNX model via the `silero` crate. Supports both offline batch detection (`detect_segments`) and streaming chunk-based detection (`forward_chunk`). |
| `EnergyVad` | RMS-energy-based fallback VAD for CPU-only or low-power environments. No model required. |
| `ContinuousCapture` | Ring buffer with VAD-based segment extraction. Push audio samples, extract speech segments. |
| `BargeInBuffer` | Accumulates audio while ASR is processing a previous segment. Drains on completion — prevents blocking while the user continues speaking. |

#### VAD Configuration

```rust
pub struct VadConfig {
    pub sample_rate: u32,      // 16000
    pub chunk_size: usize,     // 512 (32ms at 16kHz)
    pub threshold: f32,        // 0.5 — probability threshold for speech
    pub min_silence_ms: u32,   // 500 — silence to end a segment
    pub min_speech_ms: u32,    // 100 — minimum speech to start
    pub pre_padding_ms: u32,   // 100
    pub post_padding_ms: u32,  // 200
}
```

The `silero` crate provides a bundled ONNX model — no separate model download is required. VAD is always compiled (no feature flag).

---

### 4. ASR Engine

**Location:** `src/asr/mod.rs`  
**Key Dependencies:** `whisper-rs` (whisper.cpp bindings, v0.16.0)

#### Trait Design

```rust
pub trait AsrEngine: Send + Sync {
    fn load_model(&mut self, model_path: &str) -> SpellcastResult<()>;
    fn transcribe(&self, audio: &AudioBuffer) -> SpellcastResult<AsrResult>;
    fn is_ready(&self) -> bool;
}
```

#### Implementations

| Implementation | Use Case |
|---------------|----------|
| `WhisperAsr` | Production ASR via whisper.cpp |
| `NoopAsr` | Testing without model files |

#### `WhisperAsr` Pipeline

```
AudioBuffer -> to_f32() -> f32 samples
    -> whisper_rs::WhisperContext::create_state()
    -> FullParams { Greedy { best_of: 5 }, language: "en" }
    -> state.full(params, &audio_f32)
    -> state.as_iter() -> collect segments -> String
```

**Key parameters:**
- `SamplingStrategy::Greedy { best_of: 5 }` for high-quality transcription
- `language: "en"` (configurable)
- `no_timestamps: true` (we only need the text)

#### Model Loading

- Models are loaded from disk (downloaded via `scripts/download-models.sh`)
- whisper.cpp stdout/stderr output is suppressed during model load via `dup2` to `/dev/null` in `main.rs`, preventing log corruption of the terminal display
- The `WhisperContextParameters` default is used; future versions may expose context size and thread count

---

### 5. Tokenizer

**Location:** `src/tokenizer/mod.rs`  
**Sub-modules:** `src/tokenizer/tree_sitter.rs`, `src/tokenizer/code_spelling.rs`, `src/tokenizer/symbol_dictation.rs`  
**Key Dependencies:** `regex` (pattern matching), `tree-sitter` (AST-aware tokenization, always compiled)

#### Token Types

```rust
pub enum TokenType {
    Word,              // "hello", "don't"
    CodeIdentifier,    // "fooBar", "my_variable"
    Keyword,           // "if", "return", "fn"
    Punctuation,       // ".", ",", "!"
    Operator,          // "->", "=>", "::"
    Whitespace,        // " ", "\t", "\n"
    Number,            // "42", "3.14"
    StringLiteral,     // '"hello"'
    Comment,           // code comments
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

#### Tree-Sitter Integration

A `TreeSitterTokenizer` is available with 17+ language grammars (Rust, Go, Python, JavaScript, TypeScript, C, C++, Java, Bash, Markdown, JSON, TOML, YAML, HTML, CSS, SQL). Tree-sitter is always compiled — no feature flag. The tree-sitter grammar crates used are `tree-sitter-toml-ng`, `tree-sitter-sequel` (SQL), and `tree-sitter-md-025` (Markdown).

---

### 6. Phonetic Predictor

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
- **Extensible:** `add_word()` allows incremental index growth from user corrections

#### Performance

- Index build: O(n) where n is vocabulary size
- Query: O(n) scan + O(n log n) sort for top-k — acceptable for n < 10,000
- Expected query time: <10ms for a 10,000-word index

---

### 7. Explainer

**Location:** `src/explainer/mod.rs`  
**Key Dependencies:** `sha2` (hashing), `mistralrs` (LLM, always compiled), `ureq` (web search)

#### Fallback Chain

Fallback chain for `Explainer::explain(text, context)`:

1. **DB cache lookup** — look up by explanation text (hash computed internally by MemoryStore). Return cached token on hit.
2. **LLM fallback** — query local LLM (mistralrs, Qwen/Qwen3-4B, 4-bit ISQ). Return LLM result on success. Falls through if unavailable or fails.
3. **Web search** — dictionary lookup (MVP stub). Return match on hit. Future: DuckDuckGo API.
4. **Raw fallback** — return the explanation text as-is.

Result is stored in DB for future cache hits.

#### Dictionary (MVP Stub)

For the MVP, the "web search" fallback is a hardcoded `Dictionary` of ~20 concept-token pairs:
- `"a collection of items"` → `"array"`
- `"iterate over a collection"` → `"for loop"`
- `"a named block of code"` → `"function"`
- `"a key value store"` → `"hash map"`
- etc.

#### LLM Integration

The explainer uses `mistralrs` (always compiled, no feature flag) to query a local model:
- Default model: `Qwen/Qwen3-4B` (small, fast, runs on consumer GPUs)
- Auto-quantization to 4-bit (ISQ) via `mistralrs`
- Tokio runtime created on-demand for the blocking call

**Current status:** The DB lookup path is functional. The LLM path code exists but is **not yet wired into the terminal event loop** — pressing `e` in dictation mode logs "explainer not yet wired" and does not trigger the full pipeline.

---

### 8. Memory Store

**Location:** `src/memory/mod.rs`  
**Key Dependencies:** `rusqlite` (SQLite, bundled), `sha2` (hashing)

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

- **Hashed keys:** Explanations are SHA-256 hashed (truncated to 8 bytes, 16 hex chars) for lookup. The `MemoryStore::lookup_explained` and `store_explanation` methods compute the hash internally — no double-hashing by callers.
- **Usage tracking:** Both tables track `usage_count`. The `explained_tokens` table also tracks `last_used` for future LRU-style cache eviction.
- **Bundled SQLite:** The `bundled` feature of `rusqlite` ships SQLite with the binary.
- **In-memory mode:** `MemoryStore::open_in_memory()` enables fast, isolated tests.

---

### 9. Compute Backend

**Location:** `src/backend/mod.rs` (with `src/backend/multi_gpu.rs`)

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
    Auto,    // Auto-detect: CUDA → CPU
    Cuda,    // NVIDIA CUDA
    Vulkan,  // Not yet implemented, falls back to CPU
    Cpu,     // CPU-only fallback
}
```

The backend type is a **runtime** choice, selected via config (`[backend] type = "auto"`) or the `--backend` CLI flag. The compile-time `cuda`/`vulkan`/`cpu` features determine which GPU SDKs are linked; the runtime choice determines which backend to use.

The backend detection is **informational** — it identifies which GPU is available but the actual acceleration is handled by the respective libraries (`whisper-rs` for ASR, `mistralrs` for LLM) which have their own CUDA backend selection.

#### Multi-GPU

`MultiGpuManager` (in `src/backend/multi_gpu.rs`) auto-detects NVIDIA GPUs via `nvidia-smi` and supports assigning ASR to one GPU and LLM inference to another. Configured via `[backend] gpu_assignment` with `asr_device` and `llm_device` fields.

---

### 10. Configuration

**Location:** `src/config/mod.rs`  
**Format:** TOML, loaded from `~/.config/spellcast/config.toml` (XDG-compliant)

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
model_path = "~/.config/spellcast/models/ggml-base.en.bin"
language = "en"

[llm]
engine = "mistral-rs"            # LLM for explain feature
model_path = "Qwen/Qwen3-4B"     # HuggingFace model ID
max_tokens = 50

[keys]
mode_toggle = "CapsLock"        # Ctrl+Space also works (hardcoded in terminal)
caps_toggle = "Shift+CapsLock"
prev_token = "h"
next_token = "l"
redictate = "r"
delete_token = "x"
explain = "e"
kill_switch = "Ctrl+G"          # Config value (for documentation); terminal uses hardcoded Ctrl+G

[tokenizer]
mode = "heuristic"               # heuristic | tree-sitter
default_context = "prose"         # prose | code

[database]
path = "~/.config/spellcast/spellcast.db"

[languages]
primary = "en"
secondary = "none"
```

> **Note:** The `[keys]` section is defined in config for documentation purposes, but the terminal event loop currently uses hardcoded keybindings (Ctrl+Space, Ctrl+G, h/l/x/r/e, 1-3). Config-driven keybinding is a planned improvement.

#### CLI Arguments (`clap`)

| Flag | Description | Default |
|------|-------------|---------|
| `-c, --config` | Config file path | `~/.config/spellcast/config.toml` |
| `-b, --backend` | Backend override: `auto`/`cuda`/`vulkan`/`cpu` | From config |
| `-s, --shell` | Shell to spawn | `$SHELL` or `/bin/bash` |
| `-v, --verbose` | Verbose (debug) logging to file | false |
| `--check-audio` | List numbered, deduplicated input devices and exit | — |
| `--set-input-device <NAME>` | Save input device name to config and exit | — |

**Logging:** All logs go to `~/.config/spellcast/spellcast.log` (not stderr) to avoid corrupting the terminal display. The `-v` flag enables debug-level logging; default is info-level.

**Important:** If the config file doesn't exist, Spellcast falls back to defaults without error. If the file exists but is malformed, Spellcast returns a parse error and exits.

---

### 11. Error Handling

**Location:** `src/error.rs`  
**Crate:** `thiserror`

#### Error Taxonomy

```rust
pub enum SpellcastError {
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

---

### 12. Plugin System

**Location:** `src/plugin/mod.rs`  
**Key Dependencies:** None (trait-based, no external crate)

#### SpellcastPlugin Trait

```rust
pub trait SpellcastPlugin: Send {
    fn name(&self) -> &str;
    fn on_loaded(&mut self) {}
    fn on_unloaded(&mut self) {}
    fn on_token_committed(&mut self, _token: &Token, _context: &TokenContext) -> PluginAction;
    fn on_token_navigated(&mut self, _token: &Token, _direction: &str) -> PluginAction;
    fn on_explain(&mut self, _explanation: &str, _context: &str) -> Option<String>;
    fn custom_predictions(&mut self, _token: &Token) -> Vec<String>;
}
```

#### PluginManager

```rust
pub struct PluginManager {
    plugins: HashMap<String, Box<dyn SpellcastPlugin>>,
}
```

Key operations:

| Method | Description |
|--------|-------------|
| `register(plugin)` | Adds a plugin to the active map |
| `unload(name)` | Removes a plugin by name |
| `list()` | Returns names of all loaded plugins |
| `on_token_committed(token, ctx)` | Dispatches to all plugins |
| `on_explain(text, ctx)` | Dispatches, returns first match |
| `custom_predictions(token)` | Collects predictions from all plugins |

#### Built-in Plugins

| Plugin | `name()` | Description |
|--------|----------|-------------|
| `MedicalDictionaryPlugin` | `"medical-dictionary"` | Maps medical terminology (ECG, EEG, MRI, NSAID, appendectomy) |
| `CodeSymbolsPlugin` | `"code-symbols"` | Custom predictions for code symbols (lambda → `() =>`, arrow → `->`/`=>`, function → `fn`/`def`) |

**Note:** The plugin system is implemented as a library but is **not yet wired into the terminal event loop**. Plugins are not loaded or dispatched during runtime operation. CLI subcommands (`spellcast plugins ...`) do not exist.

---

### 13. Accessibility / Onboarding

**Location:** `src/accessibility.rs` (single file, not a directory)

#### Audio Feedback

The `AudioFeedback` struct plays brief beep tones on mode transitions:

| Event | Tone | Duration |
|-------|------|----------|
| Enter dictation | Rising two-tone (440 Hz → 660 Hz) | 100ms each |
| Exit dictation | Falling two-tone (660 Hz → 440 Hz) | 100ms each |
| Kill switch | Triple 880 Hz | 80ms each |
| Explain complete | Three-note chime (C5 → E5 → G5) | 150-200ms each |

Tones are emitted via terminal bell (`\x07`) and `std::thread::sleep` for timing.

#### Screen Reader

`ScreenReaderEvents` sends accessibility notifications via `spd-say` (Speech Dispatcher):

- `announce_mode(mode)` — announces mode transitions
- `announce_token(token)` — announces the current token
- Falls back to `stderr` if `spd-say` is unavailable

#### Onboarding Wizard

The `OnboardingWizard` is an 8-step interactive guide:

```rust
pub enum OnboardingStep {
    Welcome,         // Step 0
    MicrophoneTest,  // Step 1
    GpuDetection,    // Step 2
    ModelDownload,   // Step 3
    DictationTest,   // Step 4
    KeyBindings,     // Step 5
    KillSwitch,      // Step 6
    Complete,         // Step 7
}
```

The onboarding wizard key bindings step describes: Ctrl+Space or Caps Lock to toggle dictation, H/L to navigate tokens, Ctrl+G as the kill switch.

**Note:** The onboarding wizard struct exists but is **not yet wired into the main event loop** — there is no first-run detection or automatic triggering.

---

## Data Flow

### Dictation Flow

```
1. User presses Ctrl+Space -> enters Dictation mode
2. User presses Space -> starts 3-second recording (push-to-talk)
   (on background thread)
3. AudioCapture::new() -> record_duration(3.0)
4. AudioBuffer { samples: [...], sample_rate: 16000 }
5. WhisperAsr::transcribe(&buffer)
   (result sent via mpsc channel)
6. AsrResult { text: "hello world", inference_ms: 250 }
7. HeuristicTokenizer::tokenize("hello world")
8. TokenStream { tokens: [hello, " ", world], context: Prose }
9. Write text to PTY -> text appears in shell
10. Merge tokens into current stream
11. Predictor::predict("world", 3) -> ["word", "world", "work"]
12. Status bar: [DICT] @2:'world' | 1:word 2:world 3:work
```

### Explain Flow (partially implemented)

```
1. User navigates to a token (h/l keys)
2. Presses 'e' -> explain triggered
3. Currently: logs "explainer not yet wired" (TODO)
   -- Planned: --
   3a. User says an explanation
   3b. ASR transcribes -> explanation text
   3c. Explainer::explain(text, context)
       - DB lookup by hash -> hit? return cached token
       - LLM fallback (mistralrs) -> return LLM result
       - Dictionary lookup -> return match or raw text
   3d. Store result in DB
   3e. Replace current token with result
```

### Kill Switch Flow

```
1. User presses Ctrl+G
2. ModeController::toggle_kill_switch() → Mode::Killed
3. KILL_SWITCH_ENGAGED = true (AtomicBool)
4. All subsequent key events pass through to PTY unchanged
5. Status bar: [KILLED]
6. Press Ctrl+G again → Mode::Raw → Spellcast re-enabled
```

---

## Deployment Architecture

### Container Strategy

Spellcast uses a **distrobox-first** deployment model:

#### Layered Layout

- **Host** (Bazzite / Fedora Silverblue)
  - **distrobox container "spellcast-dev"**
    - Spellcast binary
    - Whisper model files (`~/.config/spellcast/models/`)
    - SQLite DB (`~/.config/spellcast/spellcast.db`)
    - Config (`~/.config/spellcast/config.toml`)
    - Log file (`~/.config/spellcast/spellcast.log`)
    - Bind mounts (automatic via distrobox):
      - `/run/user/*/pipewire-0` -> microphone access
      - NVIDIA devices -> GPU acceleration
      - `$HOME` -> config, models, DB, logs

#### Why distrobox?

| Need | Solution |
|------|----------|
| GPU passthrough | `distrobox create --nvidia` |
| Audio passthrough | Automatic PipeWire socket mount |
| User home access | Automatic `$HOME` bind mount |
| Isolation from host | Fedora container on any host |

### Filesystem Layout

```
~/.config/spellcast/
    config.toml          # User configuration (auto-generated on first run)
    spellcast.db         # SQLite memory store
    spellcast.log        # Log file (not stderr)
    models/
        ggml-base.en.bin # Whisper base model (English)
```

---

## Key Design Decisions

### 1. PTY Wrapper vs Terminal Emulator

**Decision:** Spellcast uses `portable-pty` to spawn a shell inside a PTY, rather than building a full terminal emulator.

**Rationale:** Building a terminal emulator would add enormous complexity (rendering, scrollback, fonts, Sixel, etc.) and duplicate the work of mature terminal emulators. By wrapping a PTY, Spellcast delegates all rendering to the host terminal while maintaining full control over keystroke interception and injection.

### 2. Background Thread PTY Reader with mpsc Channel

**Decision:** PTY output is read on a dedicated background thread, sent via `mpsc::channel`, and drained non-blocking in the event loop.

**Rationale:** A blocking `read()` on the PTY master would prevent the event loop from polling keyboard input. Using a background thread with a channel decouples PTY output from the event loop, allowing both to proceed independently. The event loop drains the channel with `try_recv()` (non-blocking).

### 3. Background Thread ASR with mpsc Channel

**Decision:** Audio recording and ASR transcription run on a background thread. Results are sent via `mpsc::channel<Result<String, String>>`.

**Rationale:** ASR transcription takes 100ms-3000ms depending on hardware. Running it on the main thread would freeze the terminal. The `asr_busy` flag prevents concurrent recordings while allowing the event loop to continue processing keys and rendering the status bar.

### 4. Raw ANSI Status Bar (not ratatui)

**Decision:** The status bar is rendered via raw ANSI escape sequences, not a TUI framework.

**Rationale:** ratatui was removed from dependencies (it was unused). Raw ANSI escapes (cursor save/restore, reverse video) are simpler, have zero dependencies, and keep the shell's scrollback intact. The status bar is written to the bottom row of the terminal and the cursor is restored to its original position.

### 5. Logs to File (not stderr)

**Decision:** All logging goes to `~/.config/spellcast/spellcast.log`, not stderr.

**Rationale:** Spellcast uses the alternate screen and raw mode. Any output to stderr would corrupt the terminal display. Writing logs to a file keeps the terminal clean while preserving diagnostic output.

### 6. whisper.cpp stdout/stderr Suppression

**Decision:** whisper.cpp's stdout/stderr output is suppressed during model loading via `dup2` to `/dev/null`.

**Rationale:** whisper.cpp prints progress bars and model information to stdout/stderr during model load, which would corrupt the terminal display before the alternate screen is entered. The `dup2` approach saves the original file descriptors, redirects to `/dev/null` during model load, and restores them afterward.

### 7. Compile-Time Features for GPU Only

**Decision:** Only GPU backend selection (`cuda`, `vulkan`, `cpu`) are compile-time features. All other functionality (tree-sitter, VAD, LLM, ASR) is always compiled.

**Rationale:** GPU SDKs (CUDA toolkit, Vulkan SDK) are build-time dependencies — you can't compile against CUDA without the toolkit installed. Everything else (tree-sitter grammars, Silero VAD model, mistralrs) has pure-Rust or bundled dependencies and compiles everywhere. The runtime choice of backend is a config/CLI option.

### 8. SQLite vs Custom File Format

**Decision:** SQLite via `rusqlite` with bundled feature.

**Rationale:** SQLite provides ACID transactions, indexed lookups, and schema management without introducing any external dependency. The `bundled` feature compiles SQLite into the binary.

### 9. Modes as Atomic State

**Decision:** Mode transitions are synchronous, lock-free operations on a simple `enum` field.

**Rationale:** The mode controller is accessed from a single thread (the main event loop). The kill switch uses an `AtomicBool` (accessed from signal handlers) in addition to the mode controller for signal-safe access.

---

## Security Considerations

### Threat Model

Spellcast operates as a **keystroke interceptor and microphone listener**. Its security posture must account for:

| Threat | Mitigation |
|--------|------------|
| Microphone accessed without consent | Spellcast only captures audio when in Dictation mode AND when the user explicitly triggers recording (Space key). Audio capture is never automatic. |
| Model file tampering | Model files are read-only after download; Spellcast does not execute code from model files (whisper.cpp is sandboxed in a C FFI). |
| SQLite injection | All user input is parameterized via `rusqlite::params!()`. No raw string concatenation. |
| Config file injection | TOML deserialization is schema-bound via `serde`. Unknown fields are ignored. |
| Panic in signal handler | The signal handler uses `AtomicBool` (lock-free) and is minimal. Terminal restoration is handled by `Drop`. |

### Attack Surface

1. **Audio input** — Spellcast opens the microphone device via PipeWire. A malicious user on the same machine could pipe audio to the device. Mitigation: standard Linux audio permissions (PipeWire socket).
2. **PTY I/O** — Spellcast reads and writes to a PTY. The shell inside the PTY has the same security properties as any shell session (user-level, no privilege escalation).
3. **Model files** — Large binary files (~75MB for base.en) loaded via FFI. Whisper.cpp has undergone extensive fuzzing.
4. **LLM inference** — The local model runs entirely in-process. No data leaves the machine.

---

## Performance Characteristics

| Operation | Expected Latency | Notes |
|-----------|-----------------|-------|
| Keystroke passthrough | <1ms | Crossterm raw mode → PTY write |
| Mode switch | <5μs | Single atomic flag toggle |
| Tokenization (heuristic) | <0.5ms | Regex-based on short text |
| Phonetic prediction (3 candidates) | <10ms | Linear scan + sort |
| ASR (GPU, base model, 3s audio) | 200-500ms | whisper.cpp CUDA |
| ASR (CPU, tiny model, 3s audio) | 500-3000ms | Depends on CPU |
| Explain (DB cache hit) | <2ms | SQLite lookup by hash |
| Explain (LLM, GPU, 4B model) | 500-2000ms | Qwen/Qwen3-4B, 4-bit quantized |
| Explain (dictionary fallback) | <1ms | HashMap lookup |
| PTY output read | <1ms | Non-blocking channel drain |
| Status bar render | <1ms | ANSI escape sequence |
| Startup time | 200-500ms | Config load, ASR model load |
| Memory usage (idle) | ~150MB | With base.en model loaded |

---

## Chaossynergy Integration

Spellcast's eventual home is as the core input component of **Chaossynergy**, an agent-native immutable Linux OS. The PTY wrapper in this repo is a spike of the dictation pipeline. A future **uinput injector** spike will evolve into the production integration:

| Path | Status | What it does |
|------|--------|-------------|
| PTY wrapper (this repo) | Current | Intercepts input at the terminal level for development, testing, and standalone use |
| uinput injector | Planned spike | Registers as a `/dev/uinput` virtual keyboard device, emits events system-wide |
| Chaossynergy core | Future (TBD) | Integration as a herdr plugin or standalone component |

The pipeline — audio → VAD → ASR → tokenization → injection — is the same across all paths. Only the output sink changes.

---

## Future Architecture

### Planned Improvements

| Feature | Description | Impact |
|---------|-------------|--------|
| VAD-based continuous capture | Wire `start_continuous` + VAD into the event loop for hands-free dictation | More natural dictation, no Space key needed |
| Streaming ASR | Real-time transcription via sherpa-onnx | Lower latency |
| BK-tree phonetic index | O(log n) phonetic lookup | Support for 100k+ vocabularies |
| Multi-language support | French, Spanish, German ASR | Broader audience |
| Config-driven keybindings | Read keybindings from `[keys]` config section instead of hardcoding | User customization |
| Explain feature wiring | Wire the explainer pipeline into the `e` key handler | Full explain functionality |
| Plugin system wiring | Wire `PluginManager` into the event loop | Runtime plugin dispatch |
| Dynamic plugin loading | Load plugins from `~/.config/spellcast/plugins/` at runtime | Ecosystem growth |

### Scaling Considerations

- **Vocabulary size:** The phonetic index is in-memory. At 100k words, memory usage would be ~20MB. Beyond this, consider mmap-backed or SQLite-backed phonetic indexes.
- **Concurrent sessions:** Spellcast is a single-user, single-session tool. Multiple concurrent sessions would require separate Spellcast instances.
- **Model size:** Whisper large-v3 (~3GB VRAM) runs on any modern GPU with 4GB+ VRAM. The default base.en model (~1GB VRAM) runs on virtually any GPU.
