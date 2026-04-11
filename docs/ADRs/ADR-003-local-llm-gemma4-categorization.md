# ADR-003: Local LLM (Gemma 4) for Transaction Categorization

**Status:** Accepted  
**Date:** 2026-04-10  
**Deciders:** Chris Phillipson

---

## Context

Transaction categorization is central to Finima's value proposition. Users upload raw bank extracts, and the system must automatically classify transactions into categories (food, housing, transportation, etc.) with merchant name normalization.

Most competitors (Monarch Money, Copilot) use cloud-hosted proprietary models, sending financial data to third-party APIs. This conflicts with Finima's core differentiator: **complete data sovereignty**.

## Decision

Use **Google's Gemma 4** model family, run locally via **Ollama** (primary) or **llama.cpp** (alternative), for all LLM-powered features:

- **Primary model:** `gemma-4-26B-A4B-it` (Q4_K_M quantization via GGUF) — best quality/performance trade-off for systems with 16+ GB VRAM.
- **Fallback model:** `gemma-4-E4B-it` — for resource-constrained setups (8 GB VRAM or CPU-only).
- **Integration path:** Ollama's OpenAI-compatible `/api/chat` endpoint with `tools` parameter for structured function calling.
- **Alternative path:** `llama-cpp-4` Rust crate for direct in-process inference (lower latency, higher integration complexity).

**Categorization approach — structured tool calling:**

- Define a `categorize_transaction` tool with a JSON schema specifying allowed categories, subcategory, merchant name, and confidence score.
- Batch up to 20 transactions per LLM call for throughput.
- Results with `confidence < 0.7` are flagged for user review.
- User overrides are stored in `user_category_overrides` and injected as few-shot examples in subsequent prompts (personalized learning).

**Additional LLM uses:**

- Recurring payment metadata enrichment (merchant full name, subscription vs. bill classification).
- Financial news article summarization.
- Account flow insight generation (e.g., "Your Amex payments increased 25% due to dining spending").

## Consequences

**Positive:**

- Zero financial data leaves the user's machine/network. Complete privacy.
- No per-request API cost. Unlimited categorization after model download.
- Gemma 4's native function calling produces reliable structured JSON output, reducing parsing fragility.
- Ollama abstracts model management (download, quantization, GPU offloading).
- User override feedback loop improves accuracy over time without retraining.

**Negative:**

- Requires GPU hardware for acceptable performance (~30s for 20-transaction batch on RTX 3060+). CPU inference is 5-10x slower.
- Model download is large (~15 GB for Q4_K_M). First-time setup friction.
- Gemma 4 accuracy may be lower than frontier cloud models (GPT-4, Claude) for ambiguous transactions. Mitigated: user override loop and the 0.7 confidence threshold for flagging.
- Ollama must be installed and running as a separate service. Adds Docker Compose complexity.

## Alternatives Considered

1. **Cloud LLM API (OpenAI, Anthropic, Google)** — Best accuracy but violates privacy-first design. Financial data sent to third parties. Per-token cost at scale. Rejected.
2. **Rule-based categorization** — No LLM needed but requires extensive manual rule maintenance. Poor generalization to new merchants. Rejected as primary but could be a fallback.
3. **Traditional ML (scikit-learn, XGBoost)** — Lighter weight than LLM but requires labeled training data, feature engineering, and retraining pipeline. Poorer handling of novel merchants. Rejected.
4. **Smaller models (Phi-3, TinyLlama)** — Lower resource requirements but poor function-calling support and lower accuracy on financial text. Rejected.
5. **Gemma 3 (27B)** — Predecessor model. Gemma 4 has improved function calling, better instruction following, and more efficient MoE architecture at similar VRAM. Rejected in favor of Gemma 4.
