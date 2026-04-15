# ADR-012: Tiered Self-Learning Transaction Categorization Engine

## Status

Proposed

## Date

2026-04-14

## Context

Finima categorizes bank transactions using a single-tier LLM pipeline. The current architecture sends every uncategorized transaction to a local LLM (Ollama) via tool-calling, processing ~1 transaction per 21 seconds. For a portfolio of 10,000 transactions, this takes over 58 hours — unacceptable for any real-world use.

**Target:** 10,000 transactions categorized in under 10 minutes (17 txn/s sustained).

The problem is fundamentally one of **mismatched tools**: an LLM is a sledgehammer being used to drive finishing nails. Most transactions are routine (Starbucks = coffee, Shell = gas, Netflix = streaming) and don't require generative reasoning. Only genuinely ambiguous descriptions need LLM intelligence.

## Decision

Replace the single-tier LLM pipeline with a **four-tier cascade** where each tier is faster and handles a larger percentage of transactions, with an LLM fallback only for the long tail. Integrate **RuVector** as the self-learning semantic layer that continuously improves from LLM outputs and user corrections.

### Architecture: The Categorization Cascade

```
┌─────────────────────────────────────────────────────────────────┐
│                    CATEGORIZATION ENGINE                        │
│                                                                 │
│  10,000 transactions in                                         │
│       │                                                         │
│       ▼                                                         │
│  ┌─────────────────────────────────────┐                        │
│  │  TIER 0: Merchant Lookup            │  ~0.1ms/txn            │
│  │  ─────────────────────              │  Coverage: 65-75%      │
│  │  • Exact merchant name match        │                        │
│  │  • Fuzzy match (Jaro-Winkler)       │                        │
│  │  • MCC code mapping (when avail)    │                        │
│  │  • User payee rules                 │                        │
│  └──────────┬──────────────────────────┘                        │
│             │ unmatched (25-35%)                                │
│             ▼                                                   │
│  ┌─────────────────────────────────────┐                        │
│  │  TIER 1: Pattern Engine             │  ~0.5ms/txn            │
│  │  ─────────────────────              │  Coverage: 10-15%      │
│  │  • Regex rules on descriptions      │                        │
│  │  • Keyword extraction               │                        │
│  │  • Amount-range heuristics          │                        │
│  │  • Temporal patterns                │                        │
│  └──────────┬──────────────────────────┘                        │
│             │ unmatched (15-20%)                                │
│             ▼                                                   │
│  ┌─────────────────────────────────────┐                        │
│  │  TIER 2: RuVector Semantic Search   │  ~0.1ms/txn            │
│  │  ─────────────────────              │  Coverage: 10-15%      │
│  │  • ONNX embeddings (<10ms)          │                        │
│  │  • HNSW kNN (61µs p50, 80K QPS)     │                        │
│  │  • SONA self-learning (LoRA+EWC++)  │                        │
│  │  • GNN relational patterns          │                        │
│  │  • Confidence threshold ≥ 0.85      │                        │
│  └──────────┬──────────────────────────┘                        │
│             │ low-confidence (3-8%)                             │
│             ▼                                                   │
│  ┌─────────────────────────────────────┐                        │
│  │  TIER 3: LLM Batch Inference        │  ~50ms/txn (batched)   │
│  │  ─────────────────────              │  Coverage: 3-8%        │
│  │  • Batch JSON output (not tool-call)│                        │
│  │  • Parallel requests (NUM_PARALLEL) │                        │
│  │  • Constrained decoding (json mode) │                        │
│  │  • Reduced context (num_ctx: 4096)  │                        │
│  └──────────┬──────────────────────────┘                        │
│             │                                                   │
│             ▼                                                   │
│  ┌─────────────────────────────────────┐                        │
│  │  FEEDBACK LOOP                      │                        │
│  │  ─────────────────────              │                        │
│  │  • LLM results → Tier 0 merchant DB │                        │
│  │  • LLM results → Tier 2 HNSW index  │                        │
│  │  • User corrections → all tiers     │                        │
│  │  • SONA adapts on every query       │                        │
│  └─────────────────────────────────────┘                        │
└─────────────────────────────────────────────────────────────────┘
```

### Throughput Budget (10K transactions)

| Tier | Transactions | Per-txn Latency | Wall Clock | Cumulative |
|------|-------------|-----------------|------------|------------|
| T0   | 7,000       | 0.1ms           | 0.7s       | 0.7s       |
| T1   | 1,500       | 0.5ms           | 0.75s      | 1.45s      |
| T2   | 1,000       | 0.1ms           | 0.1s       | 1.55s      |
| T3   | 500 (10 batches of 50) | ~2s/batch | 20s | **21.5s** |

**Total: ~22 seconds** — 27x under the 10-minute budget, leaving margin for cold starts, I/O, and DB writes.

### Domain Model (DDD Bounded Contexts)

```
┌──────────────────────────────┐
│  CATEGORIZATION CONTEXT      │  ← New bounded context
│                              │
│  Aggregates:                 │
│  • CategorizationJob         │  Orchestrates the cascade
│  • MerchantRegistry          │  Tier 0 lookup table
│  • PatternRuleSet            │  Tier 1 rules
│  • EmbeddingIndex            │  Tier 2 RuVector index
│  • LlmBatchJob               │  Tier 3 inference
│                              │
│  Domain Events:              │
│  • TransactionCategorized    │  Emitted per-txn after any tier
│  • BatchCompleted            │  Emitted after each tier completes
│  • MerchantLearned           │  Feedback loop event
│  • ConfidenceBelowThreshold  │  Escalation to next tier
│                              │
│  Value Objects:              │
│  • CategoryAssignment        │  (category, subcategory, confidence, source_tier)
│  • MerchantSignature         │  (normalized_name, patterns[], mcc?)
│  • EmbeddingVector           │  (f32[], model_version)
│                              │
│  Repository Ports:           │
│  • MerchantRepo (Tier 0)     │  CRUD + fuzzy search
│  • PatternRepo (Tier 1)      │  Ordered rule evaluation
│  • VectorRepo (Tier 2)       │  RuVector HNSW operations
│  • LlmPort (Tier 3)          │  Batch inference
└──────────────────────────────┘
         │
         │ TransactionCategorized events
         ▼
┌──────────────────────────────┐
│  TRANSACTION CONTEXT         │  ← Existing context
│  (finima-db)                 │
│                              │
│  • Transaction aggregate     │
│  • Persists category +       │
│    subcategory + confidence  │
│  • source_tier tracking      │
└──────────────────────────────┘
         │
         │ User corrections
         ▼
┌──────────────────────────────┐
│  LEARNING CONTEXT            │  ← New bounded context
│                              │
│  Aggregates:                 │
│  • FeedbackProcessor         │  Routes corrections to tiers
│  • TrainingDataset           │  Accumulates labeled examples
│  • ModelVersion              │  Tracks embedding model state
│                              │
│  Processes:                  │
│  • On UserCategoryOverride:  │
│    → Add to MerchantRegistry │
│    → Add to EmbeddingIndex   │
│    → Retrain patterns        │
│  • On LlmResult:             │
│    → If confidence ≥ 0.9:    │
│      Add merchant to T0      │
│    → Always: add to T2 index │
│  • SONA continuous adapt     │
│    → LoRA micro-updates      │
│    → EWC++ memory protection │
└──────────────────────────────┘
```

## Technology Mapping

| Component | Technology | Rationale |
|-----------|-----------|-----------|
| **Tier 0 store** | In-memory `HashMap` + `strsim` crate | Sub-microsecond exact match, Jaro-Winkler fuzzy at ~0.1ms |
| **Tier 0 data** | `greggles/mcc-codes` JSON + Plaid PFC taxonomy | 800+ MCC codes, community-maintained, CC-0 license |
| **Tier 1 engine** | `regex` crate with compiled `RegexSet` | Evaluate hundreds of patterns in a single pass |
| **Tier 2 embeddings** | RuVector built-in ONNX models | <10ms inference, Metal/CUDA/ANE acceleration, Rust-native |
| **Tier 2 search** | RuVector HNSW | 80K QPS, 61µs p50, SIMD-accelerated, 32x quantization |
| **Tier 2 learning** | RuVector SONA (LoRA + EWC++) | <1ms adaptation, no retraining, prevents catastrophic forgetting |
| **Tier 2 relations** | RuVector GNN | Multi-head attention on merchant→category graph |
| **Tier 3 inference** | Ollama with `qwen3:4b` | 88% tool-call accuracy, `format: "json"`, `NUM_PARALLEL=4` |
| **Tier 3 protocol** | Batch JSON (not tool-calling) | 3-5x throughput vs per-transaction tool calls |
| **Feedback bus** | Tokio `broadcast` channel | In-process event distribution, zero-copy |
| **Persistence** | PostgreSQL + existing repos | Transactions table already has category/subcategory/confidence |

## New Crate Structure

```
crates/
  finima-categorize/          ← NEW CRATE
    src/
      lib.rs                  # Public API: CategorizeEngine::run(txns)
      engine.rs               # Cascade orchestrator
      tier0/
        mod.rs                # MerchantLookup trait
        merchant_db.rs        # In-memory merchant registry
        mcc_loader.rs         # Load MCC codes from JSON
        fuzzy.rs              # Jaro-Winkler fuzzy matching
      tier1/
        mod.rs                # PatternEngine trait
        regex_engine.rs       # RegexSet-based pattern matcher
        amount_heuristic.rs   # Amount-range based rules
      tier2/
        mod.rs                # SemanticSearch trait
        ruvector_backend.rs   # RuVector HNSW + SONA integration
        embedder.rs           # ONNX text embedding
      tier3/
        mod.rs                # LlmCategorizer trait (existing, refactored)
        batch_json.rs         # JSON-array batch protocol (replaces tool-calling)
        parallel.rs           # Multi-request parallelism
      feedback/
        mod.rs                # FeedbackProcessor
        merchant_learner.rs   # LLM→T0 feedback
        vector_learner.rs     # LLM→T2 feedback
        user_correction.rs    # Manual override→all tiers
      types.rs                # CategoryAssignment, MerchantSignature, etc.
      config.rs               # Tier thresholds, batch sizes, feature flags
    data/
      mcc_codes.json          # Embedded MCC code database
      seed_merchants.json     # Initial merchant→category mappings
```

## Migration Path

### Phase 1: LLM Optimization (Week 1) — No new crate needed

1. Switch from tool-calling to batch JSON output in existing `finima-llm`
2. Add `num_ctx: 4096` to Ollama requests
3. Set `OLLAMA_NUM_PARALLEL=4` in deployment config
4. **Expected improvement:** 20-40x faster LLM inference

### Phase 2: Merchant Lookup (Week 2) — `finima-categorize` Tier 0

1. Create `finima-categorize` crate with Tier 0 only
2. Load `greggles/mcc-codes` and seed merchant database
3. Implement fuzzy matching with `strsim`
4. Wire into categorization handler: try Tier 0 first, fall through to LLM
5. **Expected improvement:** 65-75% of transactions skip the LLM entirely

### Phase 3: RuVector Integration (Week 3) — Tier 2

1. Add `ruvector` dependency to `finima-categorize`
2. Embed the 706+ already-categorized transactions as initial training data
3. Build HNSW index with category metadata
4. Wire Tier 2 between pattern engine and LLM
5. Enable SONA self-learning on every categorization query
6. **Expected improvement:** Another 10-15% skip the LLM

### Phase 4: Feedback Loop (Week 4) — Self-learning

1. Implement `FeedbackProcessor` that routes events to all tiers
2. High-confidence LLM results auto-populate Tier 0 merchant DB
3. All results feed Tier 2 embedding index
4. User corrections (payee rules, manual edits) strengthen all tiers
5. **Expected improvement:** System gets faster and more accurate over time

## LLM Tier Is Optional

The LLM tier (Tier 3) is **not a hard requirement**. The system operates
fully on Tiers 0-2 plus user corrections. When no LLM provider is
configured (`provider: "none"` in `config/llm.yaml`), Tier 3 is skipped
and transactions that cannot be categorized by the earlier tiers remain
uncategorized until the user assigns a category manually. The LLM serves
as an accelerator for cold-start scenarios and the long tail of ambiguous
descriptions, not as a dependency that the application requires to
function.

## Consequences

### Positive

- **350x throughput improvement** — from 0.05 txn/s to 17+ txn/s (22s for 10K)
- **Self-improving** — accuracy and speed increase with every batch processed
- **Graceful degradation** — if RuVector or LLM is unavailable, earlier tiers still work
- **Observable** — each categorization records its `source_tier`, enabling accuracy analysis per tier
- **Cost-efficient** — 95%+ of transactions never touch the expensive LLM tier

### Negative

- **Additional complexity** — four tiers vs one, more code to maintain
- **Cold-start delay** — Tier 0 merchant DB and Tier 2 index need bootstrapping
- **RuVector dependency** — new external dependency, though Rust-native
- **Memory footprint** — merchant DB + HNSW index + ONNX model (~200-500MB RAM)

### Risks

- **Tier 0 staleness** — merchant names change (acquisitions, rebranding). Mitigation: LLM feedback loop continuously refreshes.
- **Embedding drift** — SONA adaptations could diverge from ground truth. Mitigation: EWC++ prevents catastrophic forgetting; periodic validation against LLM labels.
- **False confidence** — a tier might assign wrong category with high confidence. Mitigation: track `source_tier` in DB; dashboard shows accuracy metrics per tier; users can always override.

## References

- Puri et al. (2018) — "Financial Transaction Classification Using Deep Learning" — DistilBERT on merchant strings, 95% accuracy
- Loukas et al. (2023) — SetFit for financial NLP, 91% with 64 examples/class
- [greggles/mcc-codes](https://github.com/greggles/mcc-codes) — Open MCC code database
- [Plaid PFC Taxonomy](https://plaid.com/documents/credit-category-taxonomy.csv) — Public category hierarchy
- [RuVector](https://github.com/ruvnet/ruvector) — High-performance vector DB with SONA self-learning
- [lintware/tool-calling-benchmark](https://github.com/lintware/tool-calling-benchmark) — LLM tool-call accuracy on Apple Silicon
- [Berkeley Function Calling Leaderboard V4](https://gorilla.cs.berkeley.edu/leaderboard.html)
