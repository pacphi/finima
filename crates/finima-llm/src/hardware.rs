//! Hardware detection for optimal model selection.
//!
//! Probes the system for GPU (CUDA, Metal) and CPU capabilities,
//! then resolves the best Gemma 4 variant and quantization for the
//! detected hardware.

use tracing::info;
#[cfg(feature = "cuda")]
use tracing::warn;

/// Detected hardware capabilities.
#[derive(Debug, Clone)]
pub struct HardwareProfile {
    pub accelerator: Accelerator,
    pub vram_mb: Option<u64>,
    pub system_ram_mb: u64,
    pub cpu_features: CpuFeatures,
}

/// The primary compute accelerator detected.
#[derive(Debug, Clone)]
pub enum Accelerator {
    /// NVIDIA GPU with CUDA support.
    Cuda {
        device_count: usize,
        compute_capability: (u32, u32),
    },
    /// Apple Silicon with Metal support (unified memory).
    Metal { unified_memory_mb: u64 },
    /// CPU-only (no GPU acceleration).
    CpuOnly,
}

/// CPU instruction set features.
#[derive(Debug, Clone, Default)]
pub struct CpuFeatures {
    pub avx2: bool,
    pub avx512: bool,
    pub neon: bool,
}

/// Result of model resolution.
#[derive(Debug, Clone)]
pub enum ModelSelection {
    /// User specified an explicit model.
    Explicit(String),
    /// Auto-selected based on hardware.
    Auto {
        model_id: String,
        gguf_file: String,
        reason: String,
    },
}

/// Detect the current hardware capabilities.
///
/// Probes for GPU (CUDA, Metal) and CPU SIMD features, then
/// returns a profile that can be used to select the optimal model.
pub fn detect_hardware() -> HardwareProfile {
    let mut sys = sysinfo::System::new();
    sys.refresh_memory();
    let system_ram_mb = sys.total_memory() / (1024 * 1024);

    let cpu_features = detect_cpu_features();

    // Try CUDA detection
    #[cfg(feature = "cuda")]
    {
        match try_detect_cuda() {
            Some((device_count, compute_cap, vram_mb)) => {
                info!(
                    device_count,
                    compute_capability = ?compute_cap,
                    vram_mb,
                    "Detected NVIDIA CUDA GPU"
                );
                return HardwareProfile {
                    accelerator: Accelerator::Cuda {
                        device_count,
                        compute_capability: compute_cap,
                    },
                    vram_mb: Some(vram_mb),
                    system_ram_mb,
                    cpu_features,
                };
            }
            None => {
                warn!("CUDA feature enabled but no CUDA GPU detected, falling back");
            }
        }
    }

    // Check for Apple Silicon with Metal
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        info!(
            unified_memory_mb = system_ram_mb,
            "Detected Apple Silicon with Metal support"
        );
        HardwareProfile {
            accelerator: Accelerator::Metal {
                unified_memory_mb: system_ram_mb,
            },
            vram_mb: Some(system_ram_mb),
            system_ram_mb,
            cpu_features,
        }
    }

    // CPU-only fallback
    #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
    {
        info!(
            system_ram_mb,
            avx2 = cpu_features.avx2,
            avx512 = cpu_features.avx512,
            "No GPU detected, using CPU-only mode"
        );
        HardwareProfile {
            accelerator: Accelerator::CpuOnly,
            vram_mb: None,
            system_ram_mb,
            cpu_features,
        }
    }
}

fn detect_cpu_features() -> CpuFeatures {
    CpuFeatures {
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        avx2: std::arch::is_x86_feature_detected!("avx2"),
        #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
        avx2: false,

        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        avx512: std::arch::is_x86_feature_detected!("avx512f"),
        #[cfg(not(any(target_arch = "x86", target_arch = "x86_64")))]
        avx512: false,

        #[cfg(target_arch = "aarch64")]
        neon: true,
        #[cfg(not(target_arch = "aarch64"))]
        neon: false,
    }
}

/// Attempt to detect CUDA devices. Returns `(device_count, compute_capability, vram_mb)`.
#[cfg(feature = "cuda")]
fn try_detect_cuda() -> Option<(usize, (u32, u32), u64)> {
    // In a real build with the cuda feature, this would use cudarc.
    // For now, provide a compile-gate stub that can be filled in when
    // mistralrs with cuda feature is available.
    None
}

/// Resolve the optimal model variant based on detected hardware.
///
/// If `user_model` is `"auto"`, selects based on available memory.
/// Otherwise, returns the user's explicit choice.
pub fn resolve_model(profile: &HardwareProfile, user_model: &str) -> ModelSelection {
    if user_model != "auto" {
        info!(model = user_model, "Using explicitly configured model");
        return ModelSelection::Explicit(user_model.to_string());
    }

    let available_mb = profile.vram_mb.unwrap_or(profile.system_ram_mb);

    let selection = if available_mb >= 16_000 {
        ModelSelection::Auto {
            model_id: "google/gemma-4-26B-A4B-it".to_string(),
            gguf_file: "gemma-4-26B-A4B-it-Q4_K_M.gguf".to_string(),
            reason: format!(
                "{} MB available — using 26B MoE for best quality",
                available_mb
            ),
        }
    } else if available_mb >= 8_000 {
        ModelSelection::Auto {
            model_id: "google/gemma-4-E4B-it".to_string(),
            gguf_file: "gemma-4-E4B-it-Q4_K_M.gguf".to_string(),
            reason: format!(
                "{} MB available — using E4B for good quality/speed balance",
                available_mb
            ),
        }
    } else {
        ModelSelection::Auto {
            model_id: "google/gemma-4-E2B-it".to_string(),
            gguf_file: "gemma-4-E2B-it-Q4_K_M.gguf".to_string(),
            reason: format!(
                "{} MB available — using E2B for resource-constrained systems",
                available_mb
            ),
        }
    };

    match &selection {
        ModelSelection::Auto {
            model_id, reason, ..
        } => {
            info!(model_id, reason, "Auto-selected model based on hardware");
        }
        ModelSelection::Explicit(m) => {
            info!(model = m, "Using explicit model");
        }
    }

    selection
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_model_explicit() {
        let profile = HardwareProfile {
            accelerator: Accelerator::CpuOnly,
            vram_mb: None,
            system_ram_mb: 8_000,
            cpu_features: CpuFeatures::default(),
        };
        match resolve_model(&profile, "my-custom-model") {
            ModelSelection::Explicit(m) => assert_eq!(m, "my-custom-model"),
            _ => panic!("Expected Explicit"),
        }
    }

    #[test]
    fn resolve_model_auto_large_vram() {
        let profile = HardwareProfile {
            accelerator: Accelerator::CpuOnly,
            vram_mb: Some(24_000),
            system_ram_mb: 32_000,
            cpu_features: CpuFeatures::default(),
        };
        match resolve_model(&profile, "auto") {
            ModelSelection::Auto { model_id, .. } => {
                assert!(model_id.contains("26B"));
            }
            _ => panic!("Expected Auto"),
        }
    }

    #[test]
    fn resolve_model_auto_medium_vram() {
        let profile = HardwareProfile {
            accelerator: Accelerator::CpuOnly,
            vram_mb: Some(10_000),
            system_ram_mb: 16_000,
            cpu_features: CpuFeatures::default(),
        };
        match resolve_model(&profile, "auto") {
            ModelSelection::Auto { model_id, .. } => {
                assert!(model_id.contains("E4B"));
            }
            _ => panic!("Expected Auto"),
        }
    }

    #[test]
    fn resolve_model_auto_low_ram() {
        let profile = HardwareProfile {
            accelerator: Accelerator::CpuOnly,
            vram_mb: None,
            system_ram_mb: 6_000,
            cpu_features: CpuFeatures::default(),
        };
        match resolve_model(&profile, "auto") {
            ModelSelection::Auto { model_id, .. } => {
                assert!(model_id.contains("E2B"));
            }
            _ => panic!("Expected Auto"),
        }
    }

    #[test]
    fn detect_hardware_returns_profile() {
        let profile = detect_hardware();
        assert!(profile.system_ram_mb > 0);
    }

    #[test]
    fn cpu_features_are_detected() {
        let features = detect_cpu_features();
        // On any modern machine, at least one of these should be populated
        // (this test just verifies no panic)
        let _ = features.avx2;
        let _ = features.neon;
    }
}
