#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
#
# download-models.sh — Download ASR and LLM models for Spellcast.
#
# This script downloads the required models into ~/.config/spellcast/models/.
#
# Models:
#   - whisper.cpp base.en (ASR, ~150MB)
#   - Qwen3-4B (LLM explain, ~2.5GB, optional)
#
# Usage:
#   ./download-models.sh [--all] [--asr-only] [--llm-only]

set -euo pipefail

MODELS_DIR="${HOME}/.config/spellcast/models"
mkdir -p "${MODELS_DIR}"

echo "=== Spellcast Model Downloader ==="
echo "Models directory: ${MODELS_DIR}"
echo ""

download_asr() {
    local model_name="ggml-base.en.bin"
    local model_path="${MODELS_DIR}/${model_name}"

    if [ -f "${model_path}" ]; then
        echo "  ✅ ASR model already exists: ${model_path}"
        return
    fi

    echo "  Downloading Whisper base.en model (~150MB)..."
    curl -L -o "${model_path}" \
        "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/${model_name}"
    echo "  ✅ ASR model downloaded to ${model_path}"
}

download_llm() {
    echo "  ℹ️  LLM model download is not automated for MVP."
    echo "  To use the LLM explain feature with mistral.rs:"
    echo ""
    echo "  The model will auto-download on first use:"
    echo "    cargo run --features llm"
    echo ""
    echo "  Or manually with mistralrs CLI:"
    echo "    mistralrs run -m Qwen/Qwen3-4B --max-tokens 50"
}

# Parse arguments
DOWNLOAD_ASR=false
DOWNLOAD_LLM=false
DO_ALL=false

if [ $# -eq 0 ]; then
    DO_ALL=true
fi

while [[ $# -gt 0 ]]; do
    case "$1" in
        --all) DO_ALL=true ;;
        --asr-only) DOWNLOAD_ASR=true ;;
        --llm-only) DOWNLOAD_LLM=true ;;
        --help)
            echo "Usage: $0 [--all] [--asr-only] [--llm-only]"
            exit 0
            ;;
        *) echo "Unknown option: $1"; exit 1 ;;
    esac
    shift
done

if [ "${DO_ALL}" = true ]; then
    DOWNLOAD_ASR=true
    DOWNLOAD_LLM=true
fi

if [ "${DOWNLOAD_ASR}" = true ]; then
    echo "--- ASR Model (Whisper base.en) ---"
    download_asr
    echo ""
fi

if [ "${DOWNLOAD_LLM}" = true ]; then
    echo "--- LLM Model (Qwen3-4B) ---"
    download_llm
    echo ""
fi

echo "=== Download complete ==="
echo ""
echo "Models in ${MODELS_DIR}:"
ls -lh "${MODELS_DIR}" 2>/dev/null || echo "  (empty)"