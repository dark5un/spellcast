# Spellcast — Research Findings

> Research conducted: 2026-07-26
> Methodology: Web search of crate registries, GitHub repos, distrobox docs, NVIDIA forums

## 1. ASR Engine: Speech-to-Text

### Candidate: `whisper-rs` (v0.16.0) — ✅ RECOMMENDED

| Aspect | Detail |
|---|---|
| Crate | `whisper-rs` 0.16.0 |
| Downloads | 757k+ total, 454k recent |
| Dependents | 80 |
| Updated | 2026-03-12 |
| Repo | https://codeberg.org/tazz4843/whisper-rs |
| License | MIT |
| CUDA support | `cuda` feature → `whisper-rs-sys/cuda` |
| Vulkan support | `vulkan` feature → `whisper-rs-sys/vulkan` |
| Metal support | `metal` feature |
| ROCm/HIP support | `hipblas` feature |
| OpenBLAS support | `openblas` feature |
| Build | Linux "just works" per docs; auto-generates bindings via build.rs |

**Justification**: whisper.cpp is the gold standard for local ASR. `whisper-rs` wraps it fully with feature-gated GPU support (CUDA, Vulkan, Metal). It exposes full inference API including full params (sample rate, language, etc.), segment iteration, and token-level timestamps. The crate is well-maintained (updated March 2026) and has 80 dependents.

### Candidate: `sherpa-onnx` (v1.13.4)

| Aspect | Detail |
|---|---|
| Crate | `sherpa-onnx` 1.13.4 |
| Downloads | 73k total, 68k recent |
| Updated | 2026-07-08 |
| License | Apache 2.0 |

Strong alternative — supports streaming ASR, VAD, TTS, speaker ID. However, `whisper-rs` has better accuracy for general dictation and is more battle-tested. `sherpa-onnx` would be a good second backend to add later for streaming/low-latency scenarios.

### Rejected: `vosk`

Rust bindings exist but are less maintained. Lower accuracy than Whisper. Not recommended for MVP.

## 2. LLM Inference Engine

### Candidate: `mistralrs` (v0.9.0) — ✅ RECOMMENDED

| Aspect | Detail |
|---|---|
| Crate | `mistralrs` 0.9.0 (latest) |
| Stars | 7,500+ |
| Contributors | 100 |
| Updated | 2026-07-23 |
| License | MIT |
| CUDA support | `cuda` feature (FlashAttention V2/V3) |
| Metal support | `metal` feature |
| Model format | HuggingFace, GGUF, auto-detect |
| Rust SDK | `cargo add mistralrs` |
| Quantization | ISQ (4-bit, 8-bit) via `with_auto_isq()` |

**Justification**: `mistralrs` is the most actively developed Rust-native LLM inference engine. It can load any HuggingFace model with zero config — ideal for the explain feature's need to run small quantized models (Qwen2.5-1.5B, Phi-3.5-mini). The Rust SDK is clean and well-documented. Prebuilt binaries available for CUDA GPUs.

### Candidate: `candle`

Rust-native ML framework by HuggingFace. Supports CUDA. Lighter weight but requires more manual model setup. Better for when you need fine-grained control over the model. `mistralrs` is more turnkey.

### Rejected: `llama.cpp` FFI directly

`llama-cpp-rs` bindings exist but `mistralrs` provides a higher-level, more ergonomic Rust API with auto-detection, chat templates, and quantization built in.

## 3. Terminal & PTY

### PTY: `portable-pty` (v0.9.0) — ✅ RECOMMENDED

| Aspect | Detail |
|---|---|
| Crate | `portable-pty` 0.9.0 |
| Part of | wezterm project |
| License | MIT |
| API | `PtySystem::openpty()` → master/slave pair, `spawn_command()` |

**Justification**: Provides a cross-platform PTY API that lets Spellcast spawn a shell as a child process, intercept all I/O, and manipulate the terminal stream. Well-tested as the foundation of wezterm. The master/slave pattern maps directly to Spellcast's architecture: slave runs the shell, master lets Spellcast inject keystrokes and read output.

### Terminal UI: `ratatui` + `crossterm` — ✅ RECOMMENDED

| Aspect | Detail |
|---|---|
| Crate | `ratatui` 0.30+ (modular workspace) |
| Version | Latest: 0.30.x (crossterm backend default) |
| Stars | 21,600+ |
| License | MIT |
| MSRV | 1.86 (for 0.30) |

**Justification**: Ratatui is the standard Rust TUI framework. The v0.30 workspace refactor improved compilation times and modularity. For Spellcast, we primarily need a status bar (mode indicator, token display, predictions) rather than a full-screen TUI. Crossterm handles event loop and raw mode. The status bar approach avoids the complexity of inline token highlighting in the terminal body.

## 4. Audio Capture

### `cpal` (v0.18.1) — ✅ RECOMMENDED

| Aspect | Detail |
|---|---|
| Crate | `cpal` 0.18.1 |
| Downloads | 15.6M+ total |
| Updated | 2026-06-07 |
| License | Apache 2.0 |
| MSRV | 1.85 |
| Linux backend | ALSA (supports PipeWire via ALSA compatibility layer) |
| Native backends v0.18 | Added long-requested native Linux backends |

**Justification**: `cpal` is the de-facto standard for cross-platform audio I/O in Rust. It handles device enumeration, stream configuration (16kHz, mono, 16-bit PCM for Whisper), and push-to-talk patterns. The v0.18 release added native Linux backends.

**Audio pipeline inside distrobox**: PipeWire socket is shared by default in distrobox. The container needs `pipewire-alsa` or `alsa-lib` to bridge. For MVP: use cpal's ALSA backend, install `alsa-lib-devel` and `pipewire-alsa` in the container. The PipeWire socket at `/run/user/$(id -u)/pipewire-0` must be bind-mounted (distrobox does this by default).

## 5. Phonetic Matching

### `rphonetic` (v3.0.6) — ✅ RECOMMENDED

| Aspect | Detail |
|---|---|
| Crate | `rphonetic` 3.0.6 |
| Algorithms | Soundex, Metaphone, Double Metaphone, Caverphone, Beider-Morse, NYSIIS, Phonex, Daitch-Mokotoff |
| License | Apache 2.0 |
| Port of | Apache commons-codec v1.15 |

**Justification**: For phonetic predictions, we need to compute phoneme-level distance between dictated words and known alternatives. `rphonetic` provides multiple algorithms — Double Metaphone is a strong candidate for English phonetic encoding. We compute edit distance between phonetic codes. For more advanced phoneme-level distance, the `phonetics` crate (IPA-based) can be added later.

### Complementary: `phonetics` crate

IPA-based phonetic distance with full phoneme-level Levenshtein. More accurate for fine-grained similarity but requires IPA transcriptions. Good future addition once we have a word→IPA mapping.

## 6. Database

### `rusqlite` (v0.40.1) — ✅ RECOMMENDED

| Aspect | Detail |
|---|---|
| Crate | `rusqlite` 0.40.1 |
| Downloads | 84M+ total |
| Dependents | 4,050+ |
| Updated | 2026-06-06 |
| License | MIT |
| Features | `bundled` (ships SQLite), `serde_json`, `blob` |

**Justification**: The gold standard for SQLite in Rust. Use `bundled` feature to avoid system SQLite dependency issues. Schema management for explained_tokens, phonetic_corrections, and settings tables.

## 7. Compute Backend (CUDA/Vulkan/CPU)

### CUDA: `cudarc` (v0.19.8) — ✅ FOR FUTURE USE

| Aspect | Detail |
|---|---|
| Crate | `cudarc` 0.19.8 |
| Downloads | 5.6M+ |
| Updated | 2026-06-19 |
| License | MIT OR Apache 2.0 |
| CUDA versions | 11.4-13.3 supported |
| Blackwell (RTX 5090) | Supported via CUDA 13.x feature flags (`cuda-13030`) |
| Loading | Dynamic loading by default — no build-time CUDA toolkit required |

**Justification**: `cudarc` is the safest and most ergonomic CUDA wrapper for Rust. We won't use it directly in the MVP (whisper-rs has its own CUDA FFI, mistralrs handles its own CUDA). But it's the right crate if we need to write custom CUDA kernels for audio pre/post-processing later.

For the MVP, CUDA acceleration is handled by:
- `whisper-rs` with `cuda` feature → whisper.cpp CUDA backend
- `mistralrs` with `cuda` feature → internal CUDA kernels
- Backend auto-detection: Spellcast checks `nvidia-smi` or CUDA driver availability at startup

### Vulkan: `wgpu` (compute) — FOR CPU/BACKUP

For the MVP, CPU fallback is sufficient. `wgpu` compute shaders can be added later for Vulkan GPU acceleration of audio processing.

## 8. RTX 5090 (Blackwell) CUDA Support

**Status**: ✅ Supported with caveats

- **NVIDIA driver**: Version 575.x+ supports RTX 5090 (Blackwell) on Linux
- **CUDA toolkit**: CUDA 12.8+ required for Blackwell architecture (sm100, sm120, sm121)
- **Fedora 42**: NVIDIA drivers available via RPM Fusion. Driver 575.57+ confirmed working with 5090.
- **cudarc**: Supports CUDA 13.3 via `cuda-13030` feature
- **whisper.cpp**: Built with CUDA 12.x or 13.x — JIT compiles PTX for Blackwell at runtime
- **mistralrs**: Prebuilt binaries for CUDA 13.x on Blackwell GPUs

**Workaround for `fedora:latest` distrobox**: If the `fedora:latest` image's CUDA packages don't support Blackwell:
1. Install NVIDIA CUDA toolkit from NVIDIA's repo inside the container
2. Or use the NVIDIA CUDA container image as the distrobox base
3. Or rely on PTX forward compatibility — whisper.cpp and mistralrs both ship PTX that JIT-compiles for Blackwell

**Recommendation**: Start with `--nvidia` flag on distrobox (binds host NVIDIA libs). Install `cuda-toolkit` from Fedora repos inside the container. If Blackwell support is missing, switch to CUDA 12.8+ from NVIDIA's official repo.

## 9. Distrobox Setup Details

### NVIDIA GPU Passthrough

- `distrobox create --nvidia` automatically bind-mounts host NVIDIA libraries
- For direct device access: `--additional-flags "--device /dev/nvidia0 --device /dev/nvidiactl --device /dev/nvidia-uvm"`
- Verify with `nvidia-smi` from inside the container
- The `--nvidia` flag is sufficient for most cases; `--device` flags provide direct GPU control

### Audio (PipeWire) Passthrough

- distrobox shares the PipeWire socket by default at `/run/user/$(id -u)/pipewire-0`
- Inside the container: install `alsa-lib-devel` and `pipewire-alsa` for ALSA compatibility
- For PipeWire native: install `pipewire-devel` and `pipewire-libs`
- Verify: `pw-record --version` or record a short audio clip

### uinput Device Access

- Host: udev rule (`/etc/udev/rules.d/99-spellcast-uinput.rules`) with `KERNEL=="uinput", MODE="0660", GROUP="input"`
- User must be in `input` group
- Container: devices shared by default via distrobox's `/dev` passthrough
- Verify: `ls -la /dev/uinput` from inside the container

## 10. Web Search for Explain Feature

### `ureq` or `reqwest` — ✅ RECOMMEND `ureq` for MVP

| Aspect | Detail |
|---|---|
| `ureq` | Simple HTTP client, blocking API, minimal deps |
| `reqwest` | Async, tokio-based, more features, heavier deps |

For the explain feature's web search fallback, `ureq` is sufficient (blocking call, simple GET to a web search API or DuckDuckGo).

## 11. Configuration & CLI

### `clap` (v4.x) + `serde` + `toml` — ✅ STANDARD CHOICES

- `clap` with `derive` feature for CLI argument parsing
- `serde` + `serde_derive` for config deserialization
- `toml` (v0.8) for config file parsing
- `dirs` for XDG config paths

## 12. Key Dependencies Summary

```toml
[dependencies]
# Core
tokio = { version = "1", features = ["full"] }
thiserror = "2"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
toml = "0.8"
dirs = "6"
clap = { version = "4", features = ["derive"] }

# Audio
cpal = "0.18"

# ASR
whisper-rs = { version = "0.16", default-features = false, features = ["cuda"] }

# LLM
mistralrs = { version = "0.9", features = ["cuda"], optional = true }

# Terminal
portable-pty = "0.9"
ratatui = "0.30"
crossterm = "0.29"

# Database
rusqlite = { version = "0.40", features = ["bundled"] }

# Phonetic
rphonetic = "3.0"

# Web
ureq = "3"

# Logging
log = "0.4"
env_logger = "0.11"

# Error handling
anyhow = "1"
```

## 13. Known Risks and Mitigations

| Risk | Impact | Mitigation |
|---|---|---|
| RTX 5090 (Blackwell) not in fedora:latest CUDA | Can't use GPU for ASR/LLM | Fall back to CPU; install CUDA 12.8+ from NVIDIA repo |
| PipeWire audio breaks in distrobox | No microphone access | Install pipewire-alsa in container; verify socket mount |
| `/dev/uinput` not accessible from container | Can't inject keystrokes | Ensure host udev rule + user in `input` group |
| whisper-rs FFI build fails | ASR unavailable | Set `WHISPER_DONT_GENERATE_BINDINGS=1`; use pregenerated bindings |
| mistralrs large binary size | Bloated build | Make it optional behind feature flag; use only for explain |
| Terminal raw mode conflicts | Stuck terminal | Always restore terminal on panic via Drop; use signal handlers |

## 14. Performance Expectations

| Operation | Expected Latency | Notes |
|---|---|---|
| Keystroke passthrough | <1ms | Crossterm raw mode → PTY write |
| ASR (GPU, short utterance) | 100-300ms | whisper.cpp CUDA with tiny/base model |
| ASR (CPU, short utterance) | 500-3000ms | Depends on CPU; tiny model preferred |
| Tokenization (heuristic) | <0.5ms | Regex-based, tiny |
| Phonetic prediction (3 candidates) | <10ms | Pre-indexed phonetic codes |
| Explain (DB hit) | <2ms | SQLite lookup by hash |
| Explain (LLM, GPU) | 500-2000ms | Small quantized model (1.5B-3B) |
| Explain (web search) | 1000-5000ms | HTTP request + parse |
| Mode switch | <5ms | Single atomic flag toggle |