# ADR-010: In-Process Inference via Candle/mistral.rs

**Status:** Accepted  
**Date:** 2026-04-11  
**Deciders:** Chris Phillipson  
**Supersedes:** Partial — extends ADR-003 (which remains valid for Ollama and Gemma 4 model choice)

---

## Context

ADR-003 established Ollama as the primary LLM backend and mentioned llama.cpp as an alternative in-process path. The in-process path was never implemented.

Ollama requires a separate running process (adds Docker Compose complexity, external dependency). The original design goal was to minimize external runtime dependencies.

HuggingFace Candle is a Rust-native ML framework (~20K GitHub stars, Apache-2.0) that enables embedding inference directly in the application binary. However, raw Candle lacks tool calling, grammar-constrained decoding, and production serving features.

mistral.rs is a production inference engine built on top of Candle by the same author (Eric Buehler) who maintains Candle's Gemma 4 support. It provides tool calling with grammar-constrained decoding, GGUF model loading, automatic hardware detection, and a high-level Rust API.

## Decision

1. Add `mistralrs` (the Rust library crate of the mistral.rs project) as an optional, feature-gated dependency in `finima-llm`.
2. Implement a `CandleClient` (implementing the existing `LlmClient` trait) that wraps the mistral.rs inference pipeline for in-process execution.
3. Add hardware detection (`HardwareProfile`) that probes CUDA, Metal, and CPU SIMD at startup and auto-selects the optimal Gemma 4 variant and quantization.
4. Keep `OllamaClient` as an alternative backend for users who prefer external model management.
5. Default `provider` changes from `"ollama"` to `"candle"` in `config/default.yaml` — fulfilling the zero-external-dependency goal.
6. Extract shared tool-call parsing into a `tool_calling` module used by both `OllamaClient` and `CandleClient`.
7. Use Cargo feature flags: `candle` (enables mistral.rs), `cuda` (NVIDIA GPU), `metal` (Apple Silicon), `ollama` (HTTP backend).

## Consequences

**Positive:**

- Zero external process dependency. Single binary deployment.
- Hardware auto-detection selects optimal model variant without user configuration.
- Grammar-constrained decoding in mistral.rs is more reliable for structured tool calling than Ollama's unconstrained generation.
- Same GGUF model files work for both Candle and Ollama backends — users can share downloads.
- Feature flags keep the default build lightweight; only compile ML dependencies when needed.

**Negative:**

- Compile times increase significantly when the `candle` feature is enabled (Candle + CUDA kernels).
- Binary size increases (~5-10 MB for the inference engine, model weights are separate).
- CUDA builds require the CUDA toolkit installed on the build machine.
- mistral.rs is a younger project than Ollama with a smaller community.
- Model loading at startup takes 5-30 seconds depending on model size.

## Alternatives Considered

1. **Raw Candle without mistral.rs** — Pure Candle has no tool calling or grammar-constrained decoding. Would require building those from scratch. Rejected: too much custom work for a financial app.
2. **llama-cpp-rs (FFI bindings to llama.cpp)** — C++ via FFI. Has GBNF grammar support. Rejected: introduces unsafe FFI boundary, requires C++ toolchain, breaks the Rust-only philosophy from ADR-001.
3. **candle-vllm** — Another Candle-based server. Has MCP support. Rejected: more server-oriented than library-oriented; mistral.rs has better tool calling ergonomics.
4. **Keep Ollama-only** — Simplest path. Rejected: doesn't fulfill the original zero-external-dependency design goal. Ollama remains as an optional fallback.
