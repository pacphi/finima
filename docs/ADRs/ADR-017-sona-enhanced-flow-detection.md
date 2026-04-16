# ADR-017: SONA-Enhanced Flow Detection

## Status

Proposed

## Date

2026-04-15

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
      └── SONA adapts LoRA weights on each query cycle
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

Add `ruvector` to `finima-analysis/Cargo.toml` as an optional feature:

```toml
[dependencies]
ruvector = { version = "0.1", optional = true }

[features]
sona = ["ruvector"]
```

This allows the system to work without ruvector installed (fallback to heuristic-only) while enabling SONA when the feature is activated.

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
