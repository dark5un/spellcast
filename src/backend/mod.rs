// SPDX-License-Identifier: Apache-2.0

//! Compute backend abstraction and auto-detection.
//!
//! Supports CUDA, Vulkan, and CPU backends.
//! Auto-detects the best available backend at startup.

pub mod multi_gpu;

use crate::config::BackendType;
use crate::error::{SpellcastError, SpellcastResult};

/// A compute backend descriptor.
#[derive(Debug, Clone)]
pub struct ComputeBackend {
    /// The type of backend.
    pub backend_type: BackendType,
    /// Human-readable description (e.g., "CUDA 12.8 on RTX 5090").
    pub description: String,
    /// Whether the backend supports GPU acceleration.
    pub is_gpu: bool,
    /// CUDA compute capability (if NVIDIA), or 0.
    pub cuda_compute_capability: (u32, u32),
}

impl ComputeBackend {
    /// Create a CPU backend.
    pub fn cpu() -> Self {
        Self {
            backend_type: BackendType::Cpu,
            description: "CPU (no GPU)".to_string(),
            is_gpu: false,
            cuda_compute_capability: (0, 0),
        }
    }
}

/// Detect the best available compute backend.
///
/// Order of detection:
/// 1. CUDA (NVIDIA GPU)
/// 2. Vulkan (generic GPU)
/// 3. CPU (fallback)
pub fn detect_backend(backend_type: &str) -> SpellcastResult<ComputeBackend> {
    match backend_type {
        "auto" => detect_auto(),
        "cuda" => detect_cuda().or_else(|_| {
            log::warn!("CUDA requested but not available, falling back to CPU");
            Ok(ComputeBackend::cpu())
        }),
        "vulkan" => {
            log::warn!("Vulkan backend not yet implemented in MVP, falling back to CPU");
            Ok(ComputeBackend::cpu())
        }
        "cpu" => Ok(ComputeBackend::cpu()),
        other => Err(SpellcastError::Backend(format!(
            "Unknown backend type: {other}"
        ))),
    }
}

/// Auto-detect: try CUDA, then Vulkan, then CPU.
fn detect_auto() -> SpellcastResult<ComputeBackend> {
    // Try CUDA first
    if let Ok(cuda) = detect_cuda() {
        log::info!("Auto-detected backend: {}", cuda.description);
        return Ok(cuda);
    }

    // Try Vulkan (stub — not yet implemented)
    log::info!("No CUDA GPU found, using CPU backend");
    Ok(ComputeBackend::cpu())
}

/// Try to detect a CUDA-capable NVIDIA GPU.
fn detect_cuda() -> SpellcastResult<ComputeBackend> {
    // Try running nvidia-smi to detect the GPU
    let output = std::process::Command::new("nvidia-smi")
        .args([
            "--query-gpu=name,compute_cap,driver_version",
            "--format=csv,noheader",
        ])
        .output()
        .map_err(|_| SpellcastError::Backend("nvidia-smi not found".to_string()))?;

    if !output.status.success() {
        return Err(SpellcastError::Backend(
            "nvidia-smi returned non-zero exit".to_string(),
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout.lines().next().unwrap_or("");

    if line.is_empty() {
        return Err(SpellcastError::Backend(
            "No NVIDIA GPU detected by nvidia-smi".to_string(),
        ));
    }

    // Parse: "NVIDIA GeForce RTX 5090, 10.0, 575.57.08"
    let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
    let gpu_name = parts.first().unwrap_or(&"Unknown GPU");
    let cap_str = parts.get(1).unwrap_or(&"0.0");
    let driver_version = parts.get(2).unwrap_or(&"unknown");

    // Parse compute capability (e.g., "10.0" → (10, 0))
    let cap_parts: Vec<&str> = cap_str.split('.').collect();
    let major: u32 = cap_parts.first().and_then(|s| s.parse().ok()).unwrap_or(0);
    let minor: u32 = cap_parts.get(1).and_then(|s| s.parse().ok()).unwrap_or(0);

    log::info!(
        "Detected NVIDIA GPU: {} (SM {}.{}, driver {})",
        gpu_name,
        major,
        minor,
        driver_version
    );

    Ok(ComputeBackend {
        backend_type: BackendType::Cuda,
        description: format!("CUDA ({}, SM {}.{})", gpu_name, major, minor),
        is_gpu: true,
        cuda_compute_capability: (major, minor),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cpu_backend() {
        let backend = ComputeBackend::cpu();
        assert!(!backend.is_gpu);
        assert_eq!(backend.cuda_compute_capability, (0, 0));
    }

    #[test]
    fn test_detect_cpu_backend() {
        let backend = detect_backend("cpu").unwrap();
        assert!(!backend.is_gpu);
    }

    #[test]
    fn test_unknown_backend() {
        let result = detect_backend("unknown");
        assert!(result.is_err());
    }

    #[test]
    fn test_cuda_fallback_to_cpu() {
        // On a system without CUDA, this should fall back to CPU.
        // On a system WITH CUDA (like the dev machine), it finds the GPU.
        // We just check that detect_backend("cuda") doesn't error.
        let result = detect_backend("cuda");
        assert!(result.is_ok());
    }
}
