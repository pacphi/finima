# ADR-017: SONA-Enhanced Flow Detection

## Status

Accepted

## Date

2026-04-15

## Implementation Status

Accepted and implemented on branch `feat/tier2-ruvector` (merged via
PR #TBD). Delivery breakdown:

- **Heuristic flow detection** — unchanged; primary path.
- **Pattern matcher** — `FlowPatternMatcher` trait with
  `StubPatternMatcher` (always) and `RuVectorPatternMatcher` (feature
  `sona`). The RuVector matcher uses HNSW keyed on the source
  account — patterns learned for account A never leak to account B.
- **Confirm / dismiss feedback** — `handlers/flows.rs::update_flow`
  upserts confirmed patterns (match-count +=1, higher confidence
  wins, preserves existing embedding) and decays confidence on
  dismissal (half-life in `FlowPatternRepo::record_dismissal`).
- **Persistence** — `flow_patterns` table (migration 021) extended
  with `description_embedding BYTEA` + `embedding_dim INTEGER`
  (migration 027). ReasoningBank state stored in
  `portfolios.sona_state` alongside Tier 2.
- **Embedder** — shares the Phase 3 `finima-embed` abstraction with
  Tier 2. Ollama and Candle both supported.
- **Operator tooling** — `bootstrap_flows` bin to seed from the
  historical confirmed flows. Metrics under `flow_pattern_*`.
- **LoRA adaptation** — deferred. Phase 0d spike showed MicroLoRA
  only mutates under multi-step trajectories with reward variance
  and explicit `engine.flush()`; the ReasoningBank retrieval signal
  is what the user stories actually depend on, so we shipped
  ReasoningBank-only. MicroLoRA adaptation is a future enhancement
  tracked in the spike memo.

See [`docs/guides/embedder.md`](../guides/embedder.md) for the
shared embedder configuration.

## Context

ADR-008 established inter-account flow detection using heuristic matching (±1% amount, ±2 day window) with keyword-based transfer detection as a fallback. While this handles explicit transfers well, it has three limitations:

1. **One-sided flows are unresolved.** When a checking account shows "AUTOPAY AMEX GOLD" but the Amex Gold card has no matching inflow (because credit card payments don't appear as inflows on the card), the flow detection creates a one-sided candidate with no target account. These are currently discarded.

2. **Description-based matching is brittle.** Transfer descriptions vary across institutions: "ONLINE TRANSFER TO SAV", "ACH XFER 4521", "AUTOPAY CC ENDING 8834". Static keyword lists miss many variants.

3. **No learning from user confirmations.** When users confirm or dismiss detected flows, that feedback isn't used to improve future detection accuracy.

## Decision

Integrate RuVector's SONA (Self-Organizing Neural Architecture) capabilities into the flow detection pipeline as an optional enhancement layer. This extends the existing heuristic matching rather than replacing it.

### Architecture

```text
detect_flows()
  │
  ├── Phase 1: Heuristic matching (existing, unchanged)
  │   └── Match outflows to inflows by amount ±1% and date ±2 days
  │
  ├── Phase 2: SONA semantic resolution (new)
  │   ├── For one-sided flows: embed the description, search for
  │   │   similar confirmed-flow descriptions to infer target account
  │   ├── For unmatched outflows with transfer-like descriptions:
  │   │   query HNSW index of known transfer patterns
  │   └── Confidence threshold: ≥ 0.80 to auto-create flow
  │
  └── Phase 3: Feedback loop (new)
      ├── User confirms flow → store embedding in HNSW with
      │   (source_account, target_account) metadata
      ├── User dismisses flow → negative signal, reduce similar
      │   pattern confidence via EWC++
      └── SONA adapts learned patterns immediately; MicroLoRA
          weights update at flush boundaries (phase 2+, see 0d)
```

### Integration Points

1. **`finima-analysis/src/flows.rs`**: Add `resolve_one_sided_flows()` function that takes one-sided `FlowCandidate`s and uses SONA to infer target accounts.

2. **`finima-analysis/src/sona.rs`** (new): SONA wrapper providing:
   - `embed_description(text) -> Vec<f32>` — generate embedding for a transaction description
   - `search_similar(embedding, k) -> Vec<FlowPattern>` — k-NN search in HNSW index
   - `store_pattern(description, source_account, target_account, confidence)` — add confirmed pattern
   - `adapt(feedback)` — trigger SONA learning from user confirmation/dismissal

3. **`finima-api/src/handlers/flows.rs`**: Update `detect_flows_handler` and `update_flow` (confirm/dismiss) to feed the SONA learning loop.

### Data Model

**Persistence model (per Phase 0c):**

- Learned retrieval patterns survive restart via `SonaEngine::coordinator().serialize_state()` (JSON) ↔ `.load_state(&json)` (bit-identical `find_patterns` output before vs. after). Store per portfolio in a new `portfolios.sona_state JSONB NULL` column, write on N confirmations or M minutes.
- LoRA adapter weights do **not** survive restart — `ruvector-sona` 0.1.9 exposes export to safetensors but no import back into an engine. Accept that LoRA re-converges from recorded trajectories after boot.
- HNSW index is rebuilt in-process at boot by inserting `flow_patterns` rows into a fresh `VectorDB` (memory-only); no second storage backend required.

**Learning-loop reality (per Phase 0d):** `force_learn()` runs the background cycle (ReasoningBank + BaseLoRA + EWC) but does NOT flush MicroLoRA. To exercise MicroLoRA we must (a) record multi-step trajectories with reward variance across steps (REINFORCE advantage must be non-zero), and (b) call `engine.flush()` at pass boundaries or accumulate ≥100 signals to trigger auto-flush. Because LoRA weights also don't persist, **Phase 1 ships ReasoningBank-only**; MicroLoRA is deferred to Phase 2+ once we have intermediate reasoning signals worth recording as trajectory steps.

New table `flow_patterns`:

```sql
CREATE TABLE flow_patterns (
    id UUID PRIMARY KEY,
    portfolio_id UUID NOT NULL REFERENCES portfolios(id) ON DELETE CASCADE,
    description_embedding BYTEA NOT NULL,
    description_text TEXT NOT NULL,
    source_account_id UUID NOT NULL REFERENCES accounts(id),
    target_account_id UUID NOT NULL REFERENCES accounts(id),
    confidence DOUBLE PRECISION NOT NULL DEFAULT 1.0,
    match_count INTEGER NOT NULL DEFAULT 1,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
```

### Dependency

Add two crates to `finima-analysis/Cargo.toml` behind an optional feature:

```toml
[dependencies]
ruvector-core = { version = "2.1", default-features = false, features = ["hnsw", "simd", "memory-only"], optional = true }
ruvector-sona = { version = "0.1", default-features = false, features = ["serde-support"], optional = true }

[features]
sona = ["dep:ruvector-core", "dep:ruvector-sona"]
```

The system still works without either crate installed (heuristic-only fallback); the `sona` feature pulls in both HNSW-backed retrieval and the self-learning layer.

> **Phase 0 spike findings (2026-04-17):**
>
> 1. **Crate naming correction.** The Rust crate is `ruvector-core` (not `ruvector`), and the self-learning crate is `ruvector-sona = "0.1.9"` — also on crates.io. The Phase 0 memo's earlier claim that SONA was unpublished was wrong.
> 2. **Version pin updated** from `0.1` to `2.1` for core; `0.1` for sona.
> 3. **API names in this ADR are illustrative.** The real surface: `ruvector_core::VectorDB` (not `HnswIndex`), `ruvector_sona::SonaEngine` via `ruvector_sona::engine::SonaEngineBuilder` (not a `SonaAdapter`). `apply_micro_lora` writes into a caller-owned `&mut [f32]` buffer rather than returning a `Vec`.
> 4. **Performance headroom.** Measured on Apple Silicon release build: trajectory ingest ~3.2 µs each, `apply_micro_lora` p99 ~1.3 µs at `hidden_dim=256`. Well inside the <10 ms flow-detection budget.
> 5. **Dep footprint is modest.** No ONNX, no reqwest, no C deps at the feature sets above.
>
> The wrapper methods (`embed_description`, `search_similar`, `store_pattern`, `adapt`) remain the integration contract; their implementations will sit on top of `VectorDB` (retrieval) and `SonaEngine` (adaptation). See [`docs/spikes/ruvector-phase0.md`](../spikes/ruvector-phase0.md) for full findings and the Phase 0b addendum.

## Consequences

### Positive

- One-sided flows (credit card payments, loan payments) can be resolved to their target accounts
- Detection accuracy improves over time as users confirm/dismiss flows
- Transfer description variants are learned rather than hard-coded
- Feature-gated: zero impact when SONA is not enabled

### Negative

- Additional dependency (ruvector) adds build complexity
- Embedding generation adds latency to flow detection (~10ms per description with ONNX)
- HNSW index must be bootstrapped from existing confirmed flows before it's useful
- Cold-start problem: new portfolios have no patterns to learn from

## References

- [ADR-008](ADR-008-inter-account-flow-detection.md) — Inter-account flow detection baseline
- [ADR-012](ADR-012-tiered-categorization-engine.md) — Tiered categorization engine (Tier 2 RuVector)
- [RuVector](https://github.com/ruvnet/ruvector) — HNSW + SONA self-learning vector DB
