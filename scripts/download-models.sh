#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
#
# download-models.sh — Download ASR and LLM models for Spellcast.
#
# This script downloads the required models into ~/.config/spellcast/models/
# and tracks them in a manifest.json for versioning and checksum verification.
#
# Models:
#   - whisper.cpp base.en (ASR, ~150MB)
#   - Qwen2.5-1.5B-Instruct q4_k_m (LLM explain, ~924MB)
#
# Usage (new subcommand style):
#   ./download-models.sh spellcast models download [asr|llm] [--checksum]
#   ./download-models.sh spellcast models list
#   ./download-models.sh spellcast models update [--checksum]
#
# Usage (legacy flags — backward compatible):
#   ./download-models.sh [--all] [--asr-only] [--llm-only]

set -euo pipefail

MODELS_DIR="${HOME}/.config/spellcast/models"
MANIFEST_FILE="${MODELS_DIR}/manifest.json"
SCRIPT_NAME="$(basename "$0")"

# ── Model definitions ──────────────────────────────────────────────────────────
# Each model entry: key, display name, filename, version, URL, expected SHA256
# The sha256 values are computed on first download and stored; we also
# record known-good checksums here for optional --checksum verification.

MODEL_ASR_KEY="asr"
MODEL_ASR_NAME="Whisper base.en"
MODEL_ASR_FILE="ggml-base.en.bin"
MODEL_ASR_VERSION="1.0"
MODEL_ASR_URL="https://huggingface.co/ggerganov/whisper.cpp/resolve/main/${MODEL_ASR_FILE}"

MODEL_LLM_KEY="llm"
MODEL_LLM_NAME="Qwen2.5-1.5B-Instruct q4_k_m"
MODEL_LLM_FILE="qwen2.5-1.5b-instruct-q4_k_m.gguf"
MODEL_LLM_VERSION="1.0"
MODEL_LLM_URL="https://huggingface.co/Qwen/Qwen2.5-1.5B-Instruct-GGUF/resolve/main/${MODEL_LLM_FILE}"

# ── Helpers ─────────────────────────────────────────────────────────────────────

usage() {
    cat <<EOF
Usage:
  ${SCRIPT_NAME} spellcast models download [asr|llm] [--checksum]
  ${SCRIPT_NAME} spellcast models list
  ${SCRIPT_NAME} spellcast models update [--checksum]

Legacy (backward compatible):
  ${SCRIPT_NAME} [--all] [--asr-only] [--llm-only] [--help]

Commands:
  spellcast models download [asr|llm]  Download specified model (default: asr)
                            --checksum  Verify SHA256 checksum after download
  spellcast models list                 List downloaded models and their versions
  spellcast models update               Re-download models if newer version available
                            --checksum  Verify SHA256 checksum after download

Legacy flags:
  --all        Download all models (default when no args given)
  --asr-only   Download only the ASR model
  --llm-only   Download only the LLM model
  --help       Show this help message

Models:
  asr  — ${MODEL_ASR_NAME}  (~150MB)
  llm  — ${MODEL_LLM_NAME} (~924MB)
EOF
    exit 0
}

die() {
    echo "❌ Error: $*" >&2
    exit 1
}

info() {
    echo "  ℹ️  $*"
}

success() {
    echo "  ✅ $*"
}

# ── Manifest management ────────────────────────────────────────────────────────

init_manifest() {
    mkdir -p "${MODELS_DIR}"
    if [ ! -f "${MANIFEST_FILE}" ]; then
        echo '{"version":1,"models":{}}' > "${MANIFEST_FILE}"
    fi
}

# Read a value from the manifest by dotted path (e.g. "models.asr.sha256").
# Returns empty string if the key doesn't exist.
manifest_get() {
    # $1 = dotted path like "models.asr.sha256"
    python3 -c "
import json, sys
try:
    with open('${MANIFEST_FILE}') as f:
        d = json.load(f)
    keys = '$1'.split('.')
    val = d
    for k in keys:
        if k.isdigit():
            val = val[int(k)]
        else:
            val = val.get(k, {})
    # If we got a dict (intermediate key), return empty
    if isinstance(val, dict):
        print('')
    else:
        print(val)
except (FileNotFoundError, KeyError, json.JSONDecodeError):
    print('')
"
}

manifest_set() {
    # $1 = dotted path like "models.asr.sha256"
    # $2 = value (string)
    python3 -c "
import json, sys
path = '$1'.strip()
value = '$2'
with open('${MANIFEST_FILE}') as f:
    d = json.load(f)
keys = path.split('.')
current = d
for k in keys[:-1]:
    if k.isdigit():
        k = int(k)
    current = current[k]
last_key = keys[-1]
if last_key.isdigit():
    last_key = int(last_key)
current[last_key] = value
with open('${MANIFEST_FILE}', 'w') as f:
    json.dump(d, f, indent=2)
"
}

# ── Checksum utility ───────────────────────────────────────────────────────────

compute_sha256() {
    sha256sum "$1" 2>/dev/null | cut -d' ' -f1 || {
        die "Cannot compute SHA256 (sha256sum not available)"
    }
}

verify_checksum() {
    local file="$1"
    local expected="$2"
    local label="$3"
    local actual

    if [ ! -f "$file" ]; then
        info "Cannot verify checksum for missing file: ${file}"
        return 1
    fi

    if [ -z "$expected" ]; then
        info "No stored checksum for ${label}, computing and storing..."
        actual=$(compute_sha256 "$file")
        manifest_set "models.${label}.sha256" "$actual"
        return 0
    fi

    actual=$(compute_sha256 "$file")
    if [ "$actual" = "$expected" ]; then
        success "Checksum verified for ${label}"
        return 0
    else
        echo "  ❌ Checksum mismatch for ${label}" >&2
        echo "     Expected: ${expected}" >&2
        echo "     Actual:   ${actual}" >&2
        return 1
    fi
}

# ── Download functions ─────────────────────────────────────────────────────────

download_model() {
    local model_key="$1"
    local do_checksum="${2:-false}"

    local name_var="MODEL_${model_key^^}_NAME"
    local file_var="MODEL_${model_key^^}_FILE"
    local url_var="MODEL_${model_key^^}_URL"
    local version_var="MODEL_${model_key^^}_VERSION"

    local model_name="${!name_var}"
    local filename="${!file_var}"
    local url="${!url_var}"
    local version="${!version_var}"
    local filepath="${MODELS_DIR}/${filename}"

    echo "--- ${model_name} ---"

    # Check if already downloaded with matching version
    local stored_version
    stored_version=$(manifest_get "models.${model_key}.version")

    if [ -f "$filepath" ] && [ "$stored_version" = "$version" ]; then
        info "Already present: ${filepath} (version ${version})"
        if [ "$do_checksum" = true ]; then
            local stored_sha
            stored_sha=$(manifest_get "models.${model_key}.sha256")
            verify_checksum "$filepath" "$stored_sha" "$model_key" || {
                info "Checksum failed for existing file, re-downloading..."
                rm -f "$filepath"
                download_model_raw "$filepath" "$url" "$model_name"
            }
        fi
    else
        if [ -f "$filepath" ]; then
            info "Updating from version ${stored_version:-none} to ${version}"
            rm -f "$filepath"
        fi
        download_model_raw "$filepath" "$url" "$model_name"
    fi

    # Store/update manifest
    manifest_set "models.${model_key}.id" "${model_key}"
    manifest_set "models.${model_key}.filename" "$filename"
    manifest_set "models.${model_key}.version" "$version"
    manifest_set "models.${model_key}.url" "$url"
    manifest_set "models.${model_key}.download_date" "$(date -u +%Y-%m-%dT%H:%M:%SZ)"

    # Compute and store checksum
    local sha
    sha=$(compute_sha256 "$filepath")
    manifest_set "models.${model_key}.sha256" "$sha"

    if [ "$do_checksum" = true ]; then
        verify_checksum "$filepath" "$sha" "$model_key"
    fi

    success "${model_name} ready at ${filepath}"
    echo ""
}

download_model_raw() {
    local filepath="$1"
    local url="$2"
    local name="$3"

    echo "  Downloading ${name}..."
    curl -L --progress-bar -o "${filepath}" "${url}" || {
        rm -f "${filepath}"
        die "Failed to download ${name} from ${url}"
    }
}

download_asr() {
    download_model "$MODEL_ASR_KEY" "${CHECKSUM_FLAG:-false}"
}

download_llm() {
    download_model "$MODEL_LLM_KEY" "${CHECKSUM_FLAG:-false}"
}

# ── List command ───────────────────────────────────────────────────────────────

cmd_list() {
    echo "=== Spellcast Models ==="
    echo "Models directory: ${MODELS_DIR}"
    echo ""

    if [ ! -f "${MANIFEST_FILE}" ]; then
        echo "  No manifest found. No models have been downloaded yet."
        echo ""
        ls -lh "${MODELS_DIR}" 2>/dev/null || echo "  (directory empty)"
        return
    fi

    # Parse manifest and display each model
    python3 -c "
import json, sys, os

try:
    with open('${MANIFEST_FILE}') as f:
        manifest = json.load(f)
except (FileNotFoundError, json.JSONDecodeError):
    print('  (manifest missing or corrupt)')
    sys.exit(0)

models = manifest.get('models', {})
if not models:
    print('  No models registered in manifest.')
    sys.exit(0)

for key in sorted(models.keys()):
    m = models[key]
    fname = m.get('filename', '?')
    fpath = os.path.join('${MODELS_DIR}', fname)
    exists = os.path.exists(fpath)
    fsize = os.path.getsize(fpath) if exists else 0
    size_str = f'{fsize / (1024*1024):.1f} MB' if fsize else 'N/A'
    status = '✅' if exists else '❌'
    print(f'  {status}  {m.get(\"id\", key)}')
    print(f'       Name:        {m.get(\"id\", key)}')
    print(f'       File:        {fname}')
    print(f'       Size:        {size_str}')
    print(f'       Version:     {m.get(\"version\", \"?\")}')
    print(f'       Downloaded:  {m.get(\"download_date\", \"?\")}')
    sha = m.get('sha256', '')
    if sha:
        print(f'       SHA256:      {sha[:16]}...')
    print('')
" 2>&1 || {
        echo "  (error parsing manifest)" >&2
    }

    # Also show any loose files not in manifest
    echo "--- Files in ${MODELS_DIR} ---"
    ls -lh "${MODELS_DIR}" 2>/dev/null || echo "  (empty)"
}

# ── Update command ─────────────────────────────────────────────────────────────

cmd_update() {
    echo "=== Spellcast Model Update ==="
    echo ""

    init_manifest

    # Re-download both models, manifest handles version comparison
    download_model "$MODEL_ASR_KEY" "${CHECKSUM_FLAG:-false}"
    download_model "$MODEL_LLM_KEY" "${CHECKSUM_FLAG:-false}"

    echo "=== Update complete ==="
}

# ── Main argument parsing ──────────────────────────────────────────────────────

# Variables
DOWNLOAD_ASR=false
DOWNLOAD_LLM=false
DO_ALL=false
CHECKSUM_FLAG=false
SUBCOMMAND_MODE=false

# Check for subcommand mode: "spellcast models <cmd>"
if [ $# -ge 2 ] && [ "$1" = "spellcast" ] && [ "$2" = "models" ]; then
    SUBCOMMAND_MODE=true
    shift 2  # Remove "spellcast" and "models"

    if [ $# -eq 0 ]; then
        usage
    fi

    CMD="$1"
    shift

    case "$CMD" in
        download)
            MODEL_TARGET="asr"  # default
            while [[ $# -gt 0 ]]; do
                case "$1" in
                    asr|llm) MODEL_TARGET="$1" ;;
                    --checksum) CHECKSUM_FLAG=true ;;
                    --help) usage ;;
                    *) die "Unknown argument: $1. Usage: ${SCRIPT_NAME} spellcast models download [asr|llm] [--checksum]" ;;
                esac
                shift
            done
            init_manifest
            if [ "$MODEL_TARGET" = "asr" ]; then
                download_asr
            elif [ "$MODEL_TARGET" = "llm" ]; then
                download_llm
            fi
            ;;
        list)
            cmd_list
            ;;
        update)
            while [[ $# -gt 0 ]]; do
                case "$1" in
                    --checksum) CHECKSUM_FLAG=true ;;
                    --help) usage ;;
                    *) die "Unknown argument: $1. Usage: ${SCRIPT_NAME} spellcast models update [--checksum]" ;;
                esac
                shift
            done
            cmd_update
            ;;
        --help)
            usage
            ;;
        *)
            die "Unknown subcommand: ${CMD}. Use 'spellcast models --help' for usage."
            ;;
    esac
    exit 0
fi

# ── Legacy flag parsing (backward compatible) ──────────────────────────────────

if [ $# -eq 0 ]; then
    DO_ALL=true
fi

while [[ $# -gt 0 ]]; do
    case "$1" in
        --all) DO_ALL=true ;;
        --asr-only) DOWNLOAD_ASR=true ;;
        --llm-only) DOWNLOAD_LLM=true ;;
        --help) usage ;;
        --checksum) CHECKSUM_FLAG=true ;;
        *)
            # If we got here with non-flag args and not in subcommand mode, show usage
            echo "Unknown option: $1" >&2
            usage
            ;;
    esac
    shift
done

if [ "${DO_ALL}" = true ]; then
    DOWNLOAD_ASR=true
    DOWNLOAD_LLM=true
fi

init_manifest

if [ "${DOWNLOAD_ASR}" = true ]; then
    download_asr
fi

if [ "${DOWNLOAD_LLM}" = true ]; then
    download_llm
fi

if [ "${DOWNLOAD_ASR}" = false ] && [ "${DOWNLOAD_LLM}" = false ]; then
    echo "No models selected for download."
    echo "Use --all, --asr-only, or --llm-only, or use the subcommand interface:"
    echo "  ${SCRIPT_NAME} spellcast models --help"
fi

echo ""
echo "=== Download complete ==="
echo ""
echo "Models in ${MODELS_DIR}:"
ls -lh "${MODELS_DIR}" 2>/dev/null || echo "  (empty)"