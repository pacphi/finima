# LLM Backend Retrofit: Candle, Ollama & Hardware-Aware Inference

> **Status:** Complete  
> **Date:** 2026-04-11  
> **Relates to:** ADR-003 (Local LLM for Categorization), ADR-010 (Candle/mistral.rs Backend), DDD-004 (Intelligence Bounded Context)

---

## Implementation Status

All items complete. 52 tests passing (44 unit + 8 integration), 9 ignored (require Ollama or model download).

| Item                                   | Status | Notes                                                                                                                                                                                                    |
| -------------------------------------- | ------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Phase 1: Provider Abstraction**      | DONE   | Feature-gated `ollama`/`candle`/`cuda`/`metal` in `finima-llm/Cargo.toml`. `sysinfo` added to workspace.                                                                                                 |
| **Phase 2: Candle In-Process Backend** | DONE   | `CandleClient` fully wired to `mistralrs` 0.8.1 API. `ModelBuilder`/`GgufModelBuilder` for model loading, `RequestBuilder` with tool calling, response conversion to shared JSON format.                 |
| **Phase 3: Hardware Detection**        | DONE   | `hardware.rs` with `HardwareProfile`, `Accelerator` enum, CPU SIMD detection, CUDA stub, Metal detection, auto model resolution. 6 tests passing.                                                        |
| **Phase 4: Tool Calling Engine**       | DONE   | `tool_calling.rs` extracts shared parsing from `OllamaClient`. Both Ollama and OpenAI response formats supported. 7 tests passing.                                                                       |
| **Config YAML**                        | DONE   | `config/llm.yaml` updated: `provider: "candle"`, `model: "auto"`, full `candle` block, removed `llamacpp`.                                                                                               |
| **Config Structs**                     | DONE   | `finima-api/src/config.rs`: `CandleConfig` struct, `LlamaCppConfig` removed, serde defaults.                                                                                                             |
| **State Provider Selection**           | DONE   | `finima-api/src/state.rs`: Three-way match (`candle`/`ollama`/`none`) with feature gates. `AppState::new()` now async.                                                                                   |
| **ADR-010**                            | DONE   | New ADR documenting Candle/mistral.rs decision at `docs/ADRs/ADR-010-candle-mistralrs-inference-backend.md`.                                                                                             |
| **DDD-004 Update**                     | DONE   | Added Backend/Provider, Hardware Profile, Model Resolution, Grammar-Constrained Decoding terms. Updated LlmClient implementations and added Hardware Detection service.                                  |
| **ADR-001 Update**                     | DONE   | `finima-llm` description updated from "Ollama + llama.cpp" to "Candle/mistral.rs + Ollama".                                                                                                              |
| **ADR-009 Update**                     | DONE   | Config example updated from llamacpp to candle provider block.                                                                                                                                           |
| **User Guide Update**                  | DONE   | Provider description updated to "Candle (in-process) or Ollama (HTTP)".                                                                                                                                  |
| **Frontend SettingsPage**              | DONE   | Provider label changed from "Ollama / llama.cpp" to "Candle / Ollama".                                                                                                                                   |
| **Legacy llamacpp Cleanup**            | DONE   | All llama.cpp/llamacpp references removed from implementation, config, and active docs. Historical plan/origin docs preserved.                                                                           |
| **Error Types**                        | DONE   | Added `ModelLoad`, `Inference`, `HardwareDetection`, `Configuration` variants to `LlmError`.                                                                                                             |
| **OllamaClient Refactor**              | DONE   | Wrapped in `#[cfg(feature = "ollama")]`; parsing delegated to shared `tool_calling` module.                                                                                                              |
| **Workspace Compilation**              | DONE   | `cargo check --workspace` and `cargo check -p finima-llm --features candle` both pass (0 errors).                                                                                                        |
| **mistral.rs Pipeline Wiring**         | DONE   | `mistralrs` 0.8.1 wired into `CandleClient`. Uses `ModelBuilder` (HF Hub + ISQ) or `GgufModelBuilder` (local GGUF). Tool defs converted to `mistralrs::Tool`, responses converted to shared JSON format. |
| **GGUF Model Download**                | DONE   | `model_download.rs` module with `download_model()`, `download_default_model()`, `is_model_cached()`, `available_models()`. Uses `hf-hub` sync API. 6 tests.                                              |
| **Integration Tests**                  | DONE   | `tests/integration.rs` with 17 tests: 8 always-run (no-LLM + hardware), 4 Ollama `#[ignore]`, 5 Candle `#[ignore]`.                                                                                      |

---

## Table of Contents

1. [Executive Summary](#1-executive-summary)
2. [Current Architecture](#2-current-architecture)
3. [Research Findings](#3-research-findings)
   - 3.1 [Candle Framework](#31-candle-framework)
   - 3.2 [Gemma 4 Local Inference](#32-gemma-4-local-inference)
   - 3.3 [GGUF vs MLX Quantization](#33-gguf-vs-mlx-quantization)
   - 3.4 [Tool Calling with Local Models](#34-tool-calling-with-local-models)
4. [Proposed Architecture](#4-proposed-architecture)
   - 4.1 [Provider Abstraction](#41-provider-abstraction)
   - 4.2 [Hardware Detection](#42-hardware-detection)
   - 4.3 [Model Resolution](#43-model-resolution)
5. [Technical Implementation Plan](#5-technical-implementation-plan)
   - 5.1 [Phase 1: Refactor Provider Abstraction](#51-phase-1-refactor-provider-abstraction)
   - 5.2 [Phase 2: Candle In-Process Backend](#52-phase-2-candle-in-process-backend)
   - 5.3 [Phase 3: Hardware Detection & Auto-Configuration](#53-phase-3-hardware-detection--auto-configuration)
   - 5.4 [Phase 4: Tool Calling Engine](#54-phase-4-tool-calling-engine)
6. [Configuration Design](#6-configuration-design)
7. [Gemma 4 Model Matrix](#7-gemma-4-model-matrix)
8. [Risk Assessment](#8-risk-assessment)
9. [Decision: Candle vs mistral.rs vs llama-cpp-rs](#9-decision-candle-vs-mistralrs-vs-llama-cpp-rs)
10. [Appendix: Research Sources](#10-appendix-research-sources)

---

## 1. Executive Summary

Finima's Intelligence bounded context (`finima-llm`) currently uses a trait-based `LlmClient` abstraction with a single concrete implementation: `OllamaClient`, which makes HTTP calls to an external Ollama process. The original design goal stated in ADR-003 was to **avoid external runtime dependencies** where possible, with llama.cpp listed as an alternative in-process backend. That in-process path was never implemented.

This document evaluates **HuggingFace Candle** as a Rust-native, in-process inference engine to fulfill the original zero-external-dependency goal, alongside the existing Ollama HTTP backend. It covers:

- **Candle integration** for embedded Gemma 4 inference without spawning a separate process
- **Hardware-aware model selection** (CUDA, Metal, CPU with AVX) to automatically serve optimized quantization formats (GGUF on all platforms, MLX-style Metal optimization on Apple Silicon)
- **Tool calling** implementation for structured output without relying on Ollama's tool protocol
- **Gemma 4 as the default model** for both Candle and Ollama backends

**Recommendation:** Use **mistral.rs** (a production inference engine built on Candle) as the in-process backend rather than raw Candle, because it provides built-in tool calling, GGUF loading, hardware auto-detection, and an ergonomic Rust API. Keep Ollama as an alternative HTTP backend for users who prefer external model management.

---

## 2. Current Architecture

### 2.1 Trait Abstraction (`finima-llm/src/client.rs`)

```rust
#[async_trait]
pub trait LlmClient: Send + Sync {
    async fn categorize_batch(&self, batch: &CategorizationBatch)
        -> Result<Vec<CategorizationResult>, LlmError>;
    async fn enrich_recurring(&self, group: &RecurringGroupCandidate)
        -> Result<RecurringEnrichment, LlmError>;
    async fn generate_insight(&self, prompt: &str)
        -> Result<String, LlmError>;
}
```

### 2.2 Implementations

| Implementation | Status                                  | Mechanism                                                                                 |
| -------------- | --------------------------------------- | ----------------------------------------------------------------------------------------- |
| `OllamaClient` | Active                                  | HTTP POST to `localhost:11434/api/chat`                                                   |
| _(none)_       | When `provider = "none"`                | No LLM loaded; Tiers 0-2 handle categorization; remaining transactions stay uncategorized |
| `llama.cpp`    | Configured in YAML, **not implemented** | `llamacpp.model_path: ""` placeholder                                                     |

### 2.3 Provider Selection (`finima-api/src/state.rs:71-81`)

```rust
let llm_client: Arc<dyn LlmClient> =
    if config.llm.provider == "ollama" && !config.llm.ollama.url.is_empty() {
        Arc::new(finima_llm::OllamaClient::new(&config.llm.ollama.url, &config.llm.ollama.model))
    } else {
        // No LLM loaded; Tiers 0-2 handle categorization
    };
```

### 2.4 Dependencies

`finima-llm` currently depends on: `reqwest`, `serde`, `serde_json`, `tokio`, `async-trait`, `tracing`, `thiserror`, `chrono`, `uuid`, `rust_decimal`, `finima-core`.

No ML/inference dependencies exist today.

### 2.5 What Works Well (Preserve)

- The `LlmClient` trait is clean and backend-agnostic
- Tool calling via JSON tool definitions (`tool_defs.rs`) with structured response parsing
- Two-tier categorization (pattern match first, LLM second)
- Retry strategy with exponential backoff
- Batch chunking with configurable size
- Confidence-based flagging (< 0.7)
- User override injection as few-shot examples

---

## 3. Research Findings

### 3.1 Candle Framework

**What it is:** A minimalist, Rust-native ML framework by HuggingFace (~20K GitHub stars, Apache-2.0). Designed for serverless inference with zero Python dependency.

**Architecture:**

```text
Layer 5: Applications (examples, Python bindings, WASM)
Layer 4: Models (candle-transformers: 80+ model implementations)
Layer 3: Core (Tensor, Device abstraction, Autograd)
Layer 2: Backends (CPU, CUDA, Metal via BackendDevice trait)
Layer 1: Kernels (Rayon, MKL, cuBLAS, Metal shaders)
```

**Key crates:**

| Crate                  | Purpose                                               |
| ---------------------- | ----------------------------------------------------- |
| `candle-core`          | Tensor ops, autodiff, device abstraction, GGUF reader |
| `candle-nn`            | Neural network layers (Linear, Conv, normalization)   |
| `candle-transformers`  | 80+ model implementations including Gemma             |
| `candle-flash-attn`    | Flash Attention v1/v2 (CUTLASS)                       |
| `candle-metal-kernels` | Metal compute shaders for Apple Silicon               |

**Gemma support in Candle:**

- Gemma v1 (2B, 7B): Full support
- Gemma 2 (2B, 9B, 27B): Full support
- Gemma 3: Full support + `quantized_gemma3`
- **Gemma 4**: Active development (commits April 2-9, 2026 by Eric Buehler). Basic model loading works. Open issues around PLE (Per-Layer Embeddings) architecture and safetensors index handling (#3443, #3448, #3457). **Not production-stable in raw Candle yet.**

**Candle does NOT provide:**

- Tool calling / function calling
- Structured output / grammar-constrained decoding
- HTTP serving layer
- Model management (pull/download/cache)
- Tokenizer from GGUF metadata (needs separate `tokenizers` crate)

**Performance note:** Reports of 3-8.5x slower than PyTorch for some workloads on CPU. GPU performance gap is smaller. For our batch-of-20-transactions use case with 60s timeout, this is acceptable.

### 3.2 Gemma 4 Local Inference

**Released:** April 2, 2026 by Google DeepMind.

| Variant       | Total Params | Active Params | Context | VRAM (Q4_K_M) | Use Case                       |
| ------------- | ------------ | ------------- | ------- | ------------- | ------------------------------ |
| E2B           | 5.1B         | 2.3B (PLE)    | 128K    | ~3.2 GB       | Edge/mobile                    |
| **E4B**       | 8B           | 4.5B (PLE)    | 128K    | **~5 GB**     | **8 GB systems (dev default)** |
| 26B A4B (MoE) | 26B          | 4B/token      | 256K    | **~15.6 GB**  | **16+ GB VRAM (prod default)** |
| 31B Dense     | 31B          | 31B           | 256K    | ~17.4 GB      | Maximum quality                |

**Architecture highlights relevant to us:**

- **Built-in function calling**: Gemma 4 natively supports tool/function calling at the model level
- **MoE efficiency**: The 26B A4B variant activates only 4B parameters per token despite having 26B total, making it fast for its quality level
- **PLE (Per-Layer Embeddings)**: E2B/E4B use per-layer conditioning for parameter efficiency

**Local runtime support:**

| Runtime          | Gemma 4 Status                     | GGUF           | Tool Calling              |
| ---------------- | ---------------------------------- | -------------- | ------------------------- |
| **Ollama**       | Full support (`ollama run gemma4`) | Yes            | Yes (OpenAI format)       |
| **llama.cpp**    | Full GGUF support, all 4 variants  | Yes            | Yes (GBNF grammars)       |
| **mistral.rs**   | Day-0 support including multimodal | Yes + UQFF/ISQ | Yes (grammar-constrained) |
| **Candle (raw)** | In progress, basic loading works   | Yes (reader)   | **No**                    |

### 3.3 GGUF vs MLX Quantization

**GGUF (Georgi Gerganov Universal Format)**

- The universal format: llama.cpp, Ollama, Candle, LM Studio, GPT4All all read it
- Self-contained: header + metadata + tokenizer + weights in one file
- Quantization families: Q2_K through Q8_K (K-quant), IQ series (imatrix), Unsloth Dynamic
- **Our current choice:** Q4_K_M for 26B A4B (good quality/speed balance)
- **Cross-platform**: Works on CUDA, Metal, CPU with SIMD

**MLX Format**

- Apple's framework for Apple Silicon unified memory
- SafeTensors-based with Apple-specific optimizations
- ~50% latency reduction vs GGUF on Apple Silicon for some models
- 13% memory savings on large models
- **Apple-only**: Cannot run on Linux/CUDA
- Candle does **not** support MLX format natively; Candle's Apple Silicon support is via Metal compute shaders

**Recommendation:** Use GGUF as the universal format. On Apple Silicon, Candle's Metal backend provides GPU acceleration without needing the MLX format. If Apple Silicon users want maximum performance, they can use Ollama (which uses llama.cpp's Metal backend) or install MLX separately -- but we should not add MLX as a Finima dependency since it would break the cross-platform goal.

### 3.4 Tool Calling with Local Models

This is the critical capability for Finima. Our `categorize_transaction` and `enrich_recurring` tools require structured JSON output with specific schemas.

**Approaches for in-process tool calling:**

| Approach                         | How                                                             | Reliability                      | Latency Impact   |
| -------------------------------- | --------------------------------------------------------------- | -------------------------------- | ---------------- |
| **Grammar-constrained decoding** | Force token generation to match JSON schema via GBNF/LLGuidance | Very high (structural guarantee) | +5-15%           |
| **Native model tool calling**    | Gemma 4's built-in function calling format                      | High (model-dependent)           | None             |
| **Post-hoc JSON parsing**        | Generate freely, parse JSON from output                         | Medium (hallucination risk)      | None             |
| **Retry with validation**        | Generate, validate, retry on failure                            | Medium-High                      | +100% worst case |

**mistral.rs tool calling implementation:**

- OpenAI-compatible `tools` parameter in requests
- Grammar-constrained decoding forces valid JSON matching the tool schema
- Strict schema mode for guaranteed structural conformance
- Server-side agentic loop: can auto-execute tools and feed results back
- Rust API: `.with_tool_callback(...)` on the pipeline builder

**This is the primary reason to use mistral.rs over raw Candle.** Raw Candle gives you logits; you'd need to build grammar-constrained decoding from scratch. mistral.rs provides it out of the box.

---

## 4. Proposed Architecture

### 4.1 Provider Abstraction

Extend the existing `LlmClient` trait to support three concrete backends:

```text
                    ┌─────────────────┐
                    │   LlmClient     │ (existing trait, unchanged)
                    │   trait          │
                    └────────┬────────┘
                             │
              ┌──────────────┼──────────────┐
              │              │              │
     ┌────────▼───────┐ ┌───▼────────┐ ┌──▼──────────────┐
     │ OllamaClient   │ │ CandleClient│ │ (none/disabled) │
     │ (HTTP, existing)│ │ (in-process)│ │ (fallback)      │
     └────────────────┘ └────────────┘ └─────────────────┘
                              │
                              │ uses
                    ┌─────────▼──────────┐
                    │ mistral.rs engine   │
                    │ (Candle + tool call │
                    │  + GGUF + hardware) │
                    └────────────────────┘
```

The `CandleClient` wraps `mistralrs` (the Rust crate of the mistral.rs project) which provides:

- Model loading from GGUF or SafeTensors
- Hardware auto-detection (CUDA, Metal, CPU+AVX)
- Token generation with grammar-constrained decoding
- Tool calling compatible with our existing tool definitions
- In-process execution (no HTTP, no external process)

### 4.2 Hardware Detection

Implement a `HardwareProfile` that detects capabilities at startup:

```rust
pub struct HardwareProfile {
    pub accelerator: Accelerator,
    pub vram_mb: Option<u64>,
    pub system_ram_mb: u64,
    pub cpu_features: CpuFeatures,
}

pub enum Accelerator {
    Cuda { device_count: usize, compute_capability: (u32, u32) },
    Metal { unified_memory_mb: u64 },
    CpuOnly,
}

pub struct CpuFeatures {
    pub avx2: bool,
    pub avx512: bool,
    pub neon: bool, // ARM
}
```

Detection uses:

- **CUDA**: `cudarc::CudaDevice::new(0)` — returns `Err` if no CUDA GPU
- **Metal**: `cfg!(target_os = "macos") && cfg!(target_arch = "aarch64")` + Metal device enumeration
- **CPU SIMD**: `is_x86_feature_detected!("avx2")`, `is_x86_feature_detected!("avx512f")`
- **RAM/VRAM**: `sysinfo` crate for system RAM; CUDA API for VRAM; Metal API for unified memory

### 4.3 Model Resolution

Given the hardware profile, automatically select the optimal model variant and quantization:

```text
┌──────────────────────────────────────────────────────────┐
│                   Model Resolution Logic                  │
├─────────────────────┬────────────────────────────────────┤
│ 16+ GB VRAM (CUDA)  │ gemma-4-26B-A4B-it Q4_K_M (GGUF) │
│ 32+ GB unified (M3+)│ gemma-4-26B-A4B-it Q4_K_M (GGUF) │
│ 8-16 GB VRAM/unified│ gemma-4-E4B-it Q4_K_M (GGUF)     │
│ < 8 GB / CPU only   │ gemma-4-E2B-it Q4_K_M (GGUF)     │
└─────────────────────┴────────────────────────────────────┘
```

Users can override via config (`llm.model` in YAML or `APP__LLM__MODEL` env var). Auto-detection is the default when `model: "auto"`.

---

## 5. Technical Implementation Plan

### 5.1 Phase 1: Refactor Provider Abstraction

**Goal:** Generalize the provider selection to support `candle` as a third option alongside `ollama` and `none`.

**Files changed:**

| File                       | Change                                           |
| -------------------------- | ------------------------------------------------ |
| `config/llm.yaml`          | Add `candle` provider config block               |
| `finima-api/src/config.rs` | Add `CandleConfig` struct                        |
| `finima-api/src/state.rs`  | Extend match on `provider` to include `"candle"` |
| `finima-llm/src/lib.rs`    | Re-export `CandleClient`                         |
| `finima-llm/Cargo.toml`    | Add feature-gated dependencies                   |

**New YAML structure:**

```yaml
llm:
  provider: 'candle' # "ollama" | "candle" | "none"
  model: 'auto' # "auto" | specific model identifier

  candle:
    model_id: 'google/gemma-4-E4B-it' # HuggingFace model ID
    model_path: '' # Local path (overrides model_id)
    quantization: 'Q4_K_M' # GGUF quant level
    device: 'auto' # "auto" | "cuda:0" | "metal" | "cpu"
    context_length: 8192 # Max context window for inference

  ollama:
    url: 'http://localhost:11434'
    model: 'gemma4:26b-a4b-it-q4_K_M'
```

**Cargo.toml feature gates:**

```toml
[features]
default = ["ollama"]
ollama = ["reqwest"]
candle = ["mistralrs", "hf-hub", "tokenizers"]
cuda = ["candle", "mistralrs/cuda"]
metal = ["candle", "mistralrs/metal"]

[dependencies]
# Always available
finima-core = { path = "../finima-core" }
serde = { workspace = true }
serde_json = { workspace = true }
tokio = { workspace = true }
tracing = { workspace = true }
thiserror = { workspace = true }
async-trait = { workspace = true }
chrono = { workspace = true }
uuid = { workspace = true }
rust_decimal = { workspace = true }

# Ollama backend (HTTP)
reqwest = { workspace = true, optional = true }

# Candle backend (in-process via mistral.rs)
mistralrs = { version = "0.4", optional = true, default-features = false }
hf-hub = { version = "0.3", optional = true }
tokenizers = { version = "0.20", optional = true }

# Hardware detection
sysinfo = "0.33"
```

### 5.2 Phase 2: Candle In-Process Backend

**Goal:** Implement `CandleClient` that loads a GGUF model in-process and runs inference with tool calling.

**New file: `finima-llm/src/candle_backend.rs`**

Pseudocode for the core implementation:

```rust
use mistralrs::{
    GGUFModelBuilder, TextModelBuilder, IsqType,
    RequestBuilder, Response, Tool, ToolChoice,
};

pub struct CandleClient {
    pipeline: Arc<MistralRs>,  // mistral.rs inference pipeline
    model_id: String,
}

impl CandleClient {
    pub async fn new(config: &CandleConfig) -> Result<Self, LlmError> {
        // 1. Detect hardware
        let device = detect_optimal_device(&config.device)?;

        // 2. Build pipeline
        let pipeline = if config.model_path.ends_with(".gguf") {
            // Load from local GGUF file
            GGUFModelBuilder::new(
                &config.model_id,
                vec![&config.model_path],
            )
            .with_device(device)
            .build()
            .await?
        } else {
            // Download from HuggingFace Hub + apply ISQ quantization
            TextModelBuilder::new(&config.model_id)
                .with_isq(IsqType::Q4K)
                .with_device(device)
                .build()
                .await?
        };

        Ok(Self { pipeline, model_id: config.model_id.clone() })
    }
}

#[async_trait]
impl LlmClient for CandleClient {
    async fn categorize_batch(&self, batch: &CategorizationBatch)
        -> Result<Vec<CategorizationResult>, LlmError>
    {
        let system_prompt = crate::prompts::build_categorization_system_prompt();
        let user_prompt = crate::prompts::build_categorization_user_prompt(
            &batch.transactions, &batch.user_overrides,
        );

        // Convert our tool_defs JSON to mistral.rs Tool objects
        let tools = vec![
            Tool::from_json(crate::tool_defs::categorize_transaction_tool())?,
        ];

        let request = RequestBuilder::new()
            .add_message(TextMessageRole::System, system_prompt)
            .add_message(TextMessageRole::User, user_prompt)
            .set_tools(tools)
            .set_tool_choice(ToolChoice::Auto);

        let response = self.pipeline.send_request(request).await?;

        // Parse tool calls from response (same format as Ollama)
        parse_categorization_response(&response, &batch.transactions)
    }

    // ... enrich_recurring and generate_insight follow same pattern
}
```

**Key implementation details:**

1. **Model loading is async and slow** (~5-30s depending on model size). Load once at application startup in `AppState::new()`, not per-request.

2. **Thread safety**: `MistralRs` pipeline is `Send + Sync` and handles internal batching. Safe behind `Arc<>`.

3. **Memory management**: GGUF models are memory-mapped. The OS pages in weights on demand. A Q4_K_M 26B model maps ~15.6 GB but actual resident memory depends on what layers are active.

4. **Response parsing**: mistral.rs returns tool calls in the same OpenAI-compatible format as Ollama. Our existing `tool_calls[].function.arguments` parsing logic in `OllamaClient` can be extracted into a shared function.

### 5.3 Phase 3: Hardware Detection & Auto-Configuration

**New file: `finima-llm/src/hardware.rs`**

```rust
pub fn detect_hardware() -> HardwareProfile {
    let system = sysinfo::System::new_all();
    let system_ram_mb = system.total_memory() / (1024 * 1024);

    // Try CUDA first
    #[cfg(feature = "cuda")]
    if let Ok(device) = cudarc::driver::CudaDevice::new(0) {
        let (major, minor) = device.compute_capability();
        let vram = device.total_memory();
        return HardwareProfile {
            accelerator: Accelerator::Cuda {
                device_count: cudarc::driver::device_count(),
                compute_capability: (major, minor),
            },
            vram_mb: Some(vram / (1024 * 1024)),
            system_ram_mb,
            cpu_features: detect_cpu_features(),
        };
    }

    // Try Metal (Apple Silicon)
    #[cfg(all(target_os = "macos", target_arch = "aarch64", feature = "metal"))]
    {
        return HardwareProfile {
            accelerator: Accelerator::Metal {
                unified_memory_mb: system_ram_mb, // unified on Apple Silicon
            },
            vram_mb: Some(system_ram_mb), // shared memory pool
            system_ram_mb,
            cpu_features: detect_cpu_features(),
        };
    }

    // CPU fallback
    HardwareProfile {
        accelerator: Accelerator::CpuOnly,
        vram_mb: None,
        system_ram_mb,
        cpu_features: detect_cpu_features(),
    }
}

fn detect_cpu_features() -> CpuFeatures {
    CpuFeatures {
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        avx2: is_x86_feature_detected!("avx2"),
        #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
        avx512: is_x86_feature_detected!("avx512f"),
        #[cfg(target_arch = "aarch64")]
        neon: true, // always available on aarch64
        ..Default::default()
    }
}

pub fn resolve_model(profile: &HardwareProfile, user_config: &str) -> ModelSelection {
    if user_config != "auto" {
        return ModelSelection::Explicit(user_config.to_string());
    }

    let available_memory_mb = profile.vram_mb.unwrap_or(profile.system_ram_mb);

    match available_memory_mb {
        m if m >= 16_000 => ModelSelection::Auto {
            model_id: "google/gemma-4-26B-A4B-it".into(),
            gguf_file: "gemma-4-26B-A4B-it-Q4_K_M.gguf".into(),
            reason: "16+ GB available, using 26B MoE for best quality".into(),
        },
        m if m >= 8_000 => ModelSelection::Auto {
            model_id: "google/gemma-4-E4B-it".into(),
            gguf_file: "gemma-4-E4B-it-Q4_K_M.gguf".into(),
            reason: "8-16 GB available, using E4B for good quality/speed balance".into(),
        },
        _ => ModelSelection::Auto {
            model_id: "google/gemma-4-E2B-it".into(),
            gguf_file: "gemma-4-E2B-it-Q4_K_M.gguf".into(),
            reason: "< 8 GB available, using E2B for resource-constrained systems".into(),
        },
    }
}
```

**Startup logging:**

```text
INFO  finima_llm::hardware: Detected hardware: Metal (Apple M3 Pro, 36 GB unified memory)
INFO  finima_llm::hardware: CPU features: NEON
INFO  finima_llm::candle_backend: Auto-selected model: gemma-4-26B-A4B-it Q4_K_M (~15.6 GB)
INFO  finima_llm::candle_backend: Loading model from HuggingFace Hub...
INFO  finima_llm::candle_backend: Model loaded in 12.3s, ready for inference
```

### 5.4 Phase 4: Tool Calling Engine

**Goal:** Extract tool-call parsing into a shared module so both `OllamaClient` and `CandleClient` use identical logic.

**New file: `finima-llm/src/tool_calling.rs`**

```rust
/// Parse categorization results from tool call responses.
/// Works with both Ollama and mistral.rs response formats (OpenAI-compatible).
pub fn parse_categorization_tool_calls(
    tool_calls: &[serde_json::Value],
    batch: &[TransactionInput],
) -> Result<Vec<CategorizationResult>, LlmError> {
    // Extracted from current OllamaClient::categorize_batch
    // Both backends produce the same tool_calls structure
    ...
}

/// Parse enrichment results from tool call responses.
pub fn parse_enrichment_tool_call(
    tool_calls: &[serde_json::Value],
) -> Result<RecurringEnrichment, LlmError> {
    // Extracted from current OllamaClient::enrich_recurring
    ...
}
```

**For the Candle/mistral.rs backend, tool calling works as follows:**

1. Our existing JSON tool definitions from `tool_defs.rs` are passed to the mistral.rs pipeline
2. mistral.rs uses grammar-constrained decoding to ensure the model outputs valid JSON matching our schema
3. The response contains `tool_calls` in the same format as Ollama
4. Our shared parsing logic extracts `category`, `subcategory`, `merchant_name`, `confidence`

This means **zero changes to our prompts or tool definitions**. The same `categorize_transaction` and `enrich_recurring` tool schemas work with both backends.

---

## 6. Configuration Design

### 6.1 Full Configuration Schema

```yaml
llm:
  # Which backend to use: "candle" (in-process), "ollama" (HTTP), "none" (no LLM)
  provider: 'candle'

  # Model selection: "auto" detects hardware and picks optimal variant
  # Or specify explicitly: "gemma-4-26B-A4B-it", "gemma-4-E4B-it", etc.
  model: 'auto'

  # Candle in-process backend configuration
  candle:
    # HuggingFace model ID (used for auto-download)
    model_id: 'google/gemma-4-E4B-it'

    # Local GGUF file path (overrides model_id download)
    model_path: ''

    # Quantization format: Q4_K_M, Q4_K_S, Q5_K_M, Q6_K, Q8_0
    quantization: 'Q4_K_M'

    # Device: "auto", "cuda:0", "cuda:1", "metal", "cpu"
    device: 'auto'

    # Maximum context length for inference
    context_length: 8192

    # Number of threads for CPU inference (0 = auto)
    threads: 0

  # Ollama HTTP backend configuration (existing)
  ollama:
    url: 'http://localhost:11434'
    model: 'gemma4:26b-a4b-it-q4_K_M'

  # Categorization tuning
  batch_size: 20
  confidence_threshold: 0.7
  timeout_seconds: 60
  max_retries: 2
```

### 6.2 Environment Variable Overrides

```bash
APP__LLM__PROVIDER=candle
APP__LLM__MODEL=auto
APP__LLM__CANDLE__MODEL_ID=google/gemma-4-E4B-it
APP__LLM__CANDLE__MODEL_PATH=/models/gemma-4-E4B-it-Q4_K_M.gguf
APP__LLM__CANDLE__DEVICE=cuda:0
```

### 6.3 Docker Considerations

```yaml
# docker-compose.yml — Candle backend (no Ollama service needed)
services:
  backend:
    build:
      context: .
      args:
        FEATURES: 'candle,cuda' # or "candle,metal" for Mac
    volumes:
      - model-cache:/app/models # Persist downloaded models
    deploy:
      resources:
        reservations:
          devices:
            - driver: nvidia
              count: 1
              capabilities: [gpu]
    environment:
      APP__LLM__PROVIDER: candle
      APP__LLM__MODEL: auto
```

With Candle, the `ollama` service in `docker-compose.yml` becomes optional. The backend binary handles inference directly.

---

## 7. Gemma 4 Model Matrix

### 7.1 Default Selection by Hardware

| Available Memory      | Model                  | Quant  | GGUF Size | Runtime Memory | Tokens/sec (est.) |
| --------------------- | ---------------------- | ------ | --------- | -------------- | ----------------- |
| 32+ GB (CUDA/Metal)   | 26B A4B MoE            | Q4_K_M | ~15.6 GB  | ~18 GB         | ~40-60 t/s        |
| 16-32 GB (CUDA/Metal) | 26B A4B MoE            | Q4_K_M | ~15.6 GB  | ~18 GB         | ~30-50 t/s        |
| 8-16 GB               | E4B (8B/4.5B active)   | Q4_K_M | ~5 GB     | ~7 GB          | ~50-80 t/s        |
| < 8 GB / CPU          | E2B (5.1B/2.3B active) | Q4_K_M | ~3.2 GB   | ~5 GB          | ~10-25 t/s        |

### 7.2 Gemma 4 for Ollama

```bash
# Ollama model tags for Gemma 4
ollama pull gemma4:26b-a4b-it-q4_K_M   # 26B MoE, 4-bit
ollama pull gemma4:e4b-it-q4_K_M       # E4B, 4-bit
ollama pull gemma4:e2b-it-q4_K_M       # E2B, 4-bit
```

Both backends default to Gemma 4. Users choose the variant based on hardware (automatic with `model: "auto"`).

### 7.3 Tool Calling Compatibility

Gemma 4 has native function calling support. Our existing tool definitions work because:

1. The model understands JSON tool schemas natively
2. Ollama passes our `tools` array directly to the model
3. mistral.rs additionally enforces schema compliance via grammar-constrained decoding

No changes needed to `tool_defs.rs` or `prompts.rs`.

---

## 8. Risk Assessment

| Risk                                | Severity | Mitigation                                                                                                                              |
| ----------------------------------- | -------- | --------------------------------------------------------------------------------------------------------------------------------------- |
| **mistral.rs Gemma 4 stability**    | Medium   | mistral.rs had day-0 Gemma 4 support. Pin to tested version. Keep Ollama as fallback.                                                   |
| **Compile times with Candle**       | Medium   | Feature-gate behind `candle` feature. Default build uses `ollama` only. CI builds both.                                                 |
| **Binary size increase**            | Low      | GGUF reader + mistral.rs adds ~5-10 MB to binary. Model weights are separate files.                                                     |
| **CUDA build complexity**           | Medium   | Requires CUDA toolkit. Document in README. Docker image with CUDA pre-installed.                                                        |
| **Model download on first run**     | Medium   | HuggingFace Hub caches models. Log progress. Allow pre-download via CLI command.                                                        |
| **Memory pressure**                 | High     | Hardware detection prevents loading oversized models. Auto-select smaller variant. Clearly log memory usage.                            |
| **Apple Silicon Metal bugs**        | Low      | Candle's Metal backend is well-tested. Fall back to CPU if Metal fails.                                                                 |
| **Tool calling quality regression** | Medium   | Grammar-constrained decoding in mistral.rs is more reliable than Ollama's unconstrained generation. Expect improvement, not regression. |
| **Candle raw Gemma 4 immaturity**   | High     | **This is why we use mistral.rs, not raw Candle.** Eric Buehler (mistral.rs author) is the same person adding Gemma 4 to Candle.        |

---

## 9. Decision: Candle vs mistral.rs vs llama-cpp-rs

### 9.1 Options Evaluated

| Criterion            | Raw Candle           | mistral.rs (on Candle)         | llama-cpp-rs (FFI)      |
| -------------------- | -------------------- | ------------------------------ | ----------------------- |
| Language             | Pure Rust            | Pure Rust                      | C++ via FFI             |
| Tool calling         | None (must build)    | Built-in (grammar-constrained) | GBNF grammars           |
| GGUF support         | Reader only          | Full pipeline                  | Native (defines format) |
| Gemma 4              | In progress          | Day-0 support                  | Full support            |
| Hardware auto-detect | Manual               | Built-in                       | Built-in                |
| API ergonomics       | Low-level tensors    | High-level pipeline            | C-style FFI             |
| Build complexity     | Moderate             | Moderate                       | Requires C++ toolchain  |
| Performance          | Good                 | Good (same engine)             | Best (C++ optimized)    |
| Memory safety        | Full Rust guarantees | Full Rust guarantees           | Unsafe FFI boundary     |
| Deployment           | Single binary        | Single binary                  | Binary + .so/.dylib     |

### 9.2 Recommendation

**Use mistral.rs as the in-process backend.**

Rationale:

1. **Tool calling is non-negotiable** for Finima's categorization pipeline. Candle doesn't have it; mistral.rs does.
2. **Same author** (Eric Buehler) maintains both Candle's Gemma 4 support and mistral.rs, ensuring fast model support.
3. **Pure Rust** — no FFI, no C++ toolchain, no unsafe boundaries. Aligns with the workspace's Rust-only philosophy.
4. **Hardware auto-detection** built in — CUDA, Metal, CPU with ISQ quantization.
5. **Superset of raw Candle** — everything Candle can do, mistral.rs can do, plus structured output and serving.
6. **GGUF compatibility** — reads the same files Ollama uses, so users can share model downloads.

The `finima-llm` crate would depend on `mistralrs` (the library crate), not the CLI/server binary. This keeps Finima as a single binary with embedded inference.

---

## 10. Appendix: Research Sources

### Candle

- [GitHub: huggingface/candle](https://github.com/huggingface/candle) — 19,953 stars, Apache-2.0
- [DeepWiki: Candle Architecture](https://deepwiki.com/huggingface/candle) — 5-layer design, backend abstraction
- [docs.rs: candle-transformers](https://docs.rs/candle-transformers/latest/candle_transformers/models/) — 80+ supported models
- [Candle Gemma 4 PR #3443](https://github.com/huggingface/candle/issues/3443) — "Implement the new Google model"
- [Candle Gemma 4 issue #3448](https://github.com/huggingface/candle/issues/3448) — Download/loading issues

### mistral.rs

- [GitHub: EricLBuehler/mistral.rs](https://github.com/EricLBuehler/mistral.rs) — Rust LLM inference engine on Candle
- [Tool Calling Docs](https://github.com/EricLBuehler/mistral.rs/blob/master/docs/TOOL_CALLING.md) — Grammar-constrained tool calling
- [candle-vllm MCP support](https://github.com/EricLBuehler/candle-vllm/blob/master/docs/mcp_tool_calling.md)

### Gemma 4

- [Google Blog: Gemma 4](https://blog.google/innovation-and-ai/technology/developers-tools/gemma-4/)
- [Gemma 4 Model Card](https://ai.google.dev/gemma/docs/core/model_card_4) — Architecture, variants, benchmarks
- [HuggingFace Blog: Gemma 4](https://huggingface.co/blog/gemma4)
- [Unsloth Gemma 4 GGUF](https://huggingface.co/unsloth/gemma-4-26B-A4B-it-GGUF) — Dynamic quantization files

### Quantization & Formats

- [GGUF Format Specification](https://github.com/ggml-org/ggml/blob/master/docs/gguf.md)
- [llama.cpp Quantize README](https://github.com/ggml-org/llama.cpp/blob/master/tools/quantize/README.md)
- [MLX vs GGUF Comparison](https://www.oreateai.com/blog/understanding-the-differences-between-gguf-and-mlx-a-comprehensive-guide/)

### Hardware Detection

- [Rust std::arch is_x86_feature_detected](https://doc.rust-lang.org/std/macro.is_x86_feature_detected.html)
- [sysinfo crate](https://docs.rs/sysinfo/latest/sysinfo/) — Cross-platform system information

### Tool Calling with Local Models

- [Local LLMs on Tool Calling 2026 Evaluation](https://www.jdhodges.com/blog/local-llms-on-tool-calling-2026-pt1-local-lm/)
- [llama.cpp GBNF Grammars](https://github.com/ggml-org/llama.cpp/blob/master/grammars/README.md)
