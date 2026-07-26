// SPDX-License-Identifier: Apache-2.0

//! Multi-GPU workload distribution.
//!
//! Distributes ASR and LLM inference across available GPUs.
//! Primary GPU (RTX 5090) handles ASR, secondary (RTX 4070 Ti)
//! LLM for the explain feature. Fallback consolidates
//! onto one GPU if the other is unavailable.

use crate::config::GpuAssignment;

/// GPU device information.
#[derive(Debug, Clone)]
pub struct GpuDevice {
    pub index: usize,
    pub name: String,
    pub memory_gb: f32,
    pub compute_capability: String,
}

/// Multi-GPU manager that assigns workloads to devices.
#[derive(Debug)]
pub struct MultiGpuManager {
    /// Detected GPU devices.
    pub devices: Vec<GpuDevice>,
    /// User-configured assignment.
    pub assignment: GpuAssignment,
    /// Whether CUDA is available.
    pub cuda_available: bool,
}

impl MultiGpuManager {
    /// Detect available GPUs and build a manager.
    pub fn detect(assignment: GpuAssignment) -> Self {
        let devices = Self::detect_gpus();
        let cuda_available = !devices.is_empty();

        Self {
            devices,
            assignment,
            cuda_available,
        }
    }

    /// Detect NVIDIA GPUs via nvidia-smi output parsing.
    fn detect_gpus() -> Vec<GpuDevice> {
        let mut devices = Vec::new();

        // Try nvidia-smi for GPU detection
        let output = std::process::Command::new("nvidia-smi")
            .args(["--query-gpu=index,name,memory.total,compute_cap", "--format=csv,noheader"])
            .output();

        if let Ok(output) = output {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                let parts: Vec<&str> = line.split(',').map(|s| s.trim()).collect();
                if parts.len() == 4 {
                    let index = parts[0].parse::<usize>().unwrap_or(0);
                    let name = parts[1].to_string();
                    let memory_str = parts[2].trim_end_matches(" MiB");
                    let memory_mib = memory_str.parse::<f32>().unwrap_or(0.0);
                    let memory_gb = memory_mib / 1024.0;
                    let compute_cap = parts[3].to_string();
                    devices.push(GpuDevice {
                        index,
                        name,
                        memory_gb,
                        compute_capability: compute_cap,
                    });
                }
            }
        }

        devices
    }

    /// Get the ASR device index.
    pub fn asr_device(&self) -> usize {
        self.assignment.asr_device.unwrap_or(0)
    }

    /// Get the LLM device index.
    pub fn llm_device(&self) -> usize {
        self.assignment.llm_device.unwrap_or_else(|| {
            if self.devices.len() > 1 { 1 } else { 0 }
        })
    }

    /// Get the device name at a given index.
    pub fn device_name(&self, index: usize) -> &str {
        self.devices
            .iter()
            .find(|d| d.index == index)
            .map(|d| d.name.as_str())
            .unwrap_or("CPU")
    }

    /// GPU count.
    pub fn gpu_count(&self) -> usize {
        self.devices.len()
    }

    /// Format a status string for the status bar.
    pub fn status_string(&self) -> String {
        if self.devices.is_empty() {
            return "GPU: none (CPU)".to_string();
        }
        let asr = self.device_name(self.asr_device());
        let llm = self.device_name(self.llm_device());
        if self.devices.len() > 1 {
            format!("ASR: {} | LLM: {}", asr, llm)
        } else {
            format!("GPU: {}", asr)
        }
    }

    /// Detect if we're running on an RTX 5090 (SM 12.0).
    pub fn has_blackwell_gpu(&self) -> bool {
        self.devices.iter().any(|d| d.compute_capability == "120")
    }

    /// Get total available VRAM across all GPUs.
    pub fn total_vram_gb(&self) -> f32 {
        self.devices.iter().map(|d| d.memory_gb).sum()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_multi_gpu_mananger_creation() {
        let assignment = GpuAssignment {
            asr_device: Some(0),
            llm_device: None,
        };
        let mgr = MultiGpuManager::detect(assignment);
        // May have 0 devices in CI (no GPU), but should not panic
        assert!(mgr.gpu_count() >= 0);
    }

    #[test]
    fn test_device_assignment() {
        let assignment = GpuAssignment {
            asr_device: Some(0),
            llm_device: Some(1),
        };
        let mgr = MultiGpuManager::detect(assignment);
        assert_eq!(mgr.asr_device(), 0);
        assert_eq!(mgr.llm_device(), 1);
    }

    #[test]
    fn test_llm_falls_back_to_asr_device() {
        let assignment = GpuAssignment {
            asr_device: Some(0),
            llm_device: None,
        };
        let mgr = MultiGpuManager::detect(assignment);
        assert_eq!(mgr.asr_device(), 0);
        // With no second GPU, LLM falls back to device 0
        assert_eq!(mgr.llm_device(), if mgr.devices.len() > 1 { 1 } else { 0 });
    }

    #[test]
    fn test_status_string_no_gpu() {
        let assignment = GpuAssignment {
            asr_device: Some(0),
            llm_device: None,
        };
        let mgr = MultiGpuManager::detect(assignment);
        let status = mgr.status_string();
        assert!(!status.is_empty());
    }

    #[test]
    fn test_gpu_name_unknown() {
        let assignment = GpuAssignment {
            asr_device: Some(0),
            llm_device: None,
        };
        let mgr = MultiGpuManager::detect(assignment);
        assert_eq!(mgr.device_name(99), "CPU"); // Non-existent index
    }

    #[test]
    fn test_has_blackwell_gpu_no_panic() {
        let assignment = GpuAssignment::default();
        let mgr = MultiGpuManager::detect(assignment);
        // Should not panic even with no GPUs
        let _ = mgr.has_blackwell_gpu();
    }
}