# Technical Plan: Tiered Self-Learning Transaction Categorization Engine

> **ADR:** [ADR-012](../ADRs/ADR-012-tiered-categorization-engine.md)
> **Goal:** 10,000 transactions categorized in under 10 minutes
> **Current:** ~0.05 txn/s (21s per transaction via single-tier LLM)
> **Target:** 17+ txn/s sustained (~22s for 10K batch)

---

## 1. Domain-Driven Design

### Bounded Contexts

The categorization engine introduces two new bounded contexts alongside the existing Transaction context.

#### 1.1 Categorization Context (`finima-categorize`)

**Responsibility:** Execute the four-tier cascade and produce `CategoryAssignment` values for uncategorized transactions.

**Aggregate: `CategorizationJob`**
```rust
pub struct CategorizationJob {
    id: Uuid,
    account_id: Uuid,
    transactions: Vec<UncategorizedTransaction>,
    assignments: Vec<CategoryAssignment>,
    tier_stats: TierStats,
    status: JobStatus,
    started_at: DateTime<Utc>,
    completed_at: Option<DateTime<Utc>>,
}

pub struct CategoryAssignment {
    transaction_id: Uuid,
    category: String,
    subcategory: String,
    merchant_name: String,
    confidence: f64,
    source_tier: CategorizationTier,
}

pub enum CategorizationTier {
    MerchantLookup,   // Tier 0
    PatternEngine,    // Tier 1
    SemanticSearch,   // Tier 2 (RuVector)
    LlmInference,    // Tier 3
    UserOverride,     // Manual correction
}

pub struct TierStats {
    pub tier0_matched: usize,
    pub tier1_matched: usize,
    pub tier2_matched: usize,
    pub tier3_matched: usize,
    pub tier3_failed: usize,
    pub total: usize,
    pub elapsed_ms: u64,
}
```

**Aggregate: `MerchantRegistry`**
```rust
pub struct MerchantRegistry {
    /// Exact name → (category, subcategory) for O(1) lookup
    exact_map: HashMap<String, MerchantEntry>,
    /// Fuzzy matching candidates indexed by first 3 chars
    prefix_index: HashMap<String, Vec<MerchantEntry>>,
    /// MCC code → category mapping
    mcc_map: HashMap<u16, CategoryMapping>,
}

pub struct MerchantEntry {
    pub canonical_name: String,
    pub aliases: Vec<String>,
    pub category: String,
    pub subcategory: String,
    pub confidence: f64,
    pub source: MerchantSource,
    pub last_seen: DateTime<Utc>,
}

pub enum MerchantSource {
    MccDatabase,      // From greggles/mcc-codes
    SeedData,         // From seed_merchants.json
    LlmLearned,      // Auto-populated from Tier 3 results
    UserDefined,      // From payee rules / manual override
}
```

**Domain Events:**
```rust
pub enum CategorizationEvent {
    /// Emitted when any tier assigns a category to a transaction.
    TransactionCategorized {
        transaction_id: Uuid,
        assignment: CategoryAssignment,
    },
    /// Emitted after each tier completes processing its batch.
    TierCompleted {
        job_id: Uuid,
        tier: CategorizationTier,
        matched: usize,
        remaining: usize,
        elapsed_ms: u64,
    },
    /// Emitted when a new merchant is learned from LLM or user feedback.
    MerchantLearned {
        merchant_name: String,
        category: String,
        subcategory: String,
        source: MerchantSource,
    },
    /// Emitted when confidence is too low and transaction escalates.
    EscalatedToNextTier {
        transaction_id: Uuid,
        from_tier: CategorizationTier,
        confidence: f64,
    },
}
```

#### 1.2 Learning Context

**Responsibility:** Process feedback signals from user corrections and LLM results to continuously improve all tiers.

**Aggregate: `FeedbackProcessor`**
```rust
pub struct FeedbackProcessor {
    merchant_registry: Arc<RwLock<MerchantRegistry>>,
    vector_index: Arc<RuVectorIndex>,
    pattern_engine: Arc<RwLock<PatternEngine>>,
}

impl FeedbackProcessor {
    /// Called when LLM (Tier 3) produces a result.
    /// High-confidence results get promoted to Tier 0.
    pub async fn on_llm_result(&self, assignment: &CategoryAssignment) {
        // Always add to Tier 2 embedding index
        self.vector_index.upsert_embedding(assignment).await;

        // Promote to Tier 0 if confidence >= 0.9
        if assignment.confidence >= 0.9 {
            self.merchant_registry.write().await
                .add_learned_merchant(assignment);
        }
    }

    /// Called when a user manually corrects a category.
    /// Corrections have the highest authority and update all tiers.
    pub async fn on_user_correction(&self, correction: &UserCorrection) {
        self.merchant_registry.write().await
            .add_user_merchant(correction);
        self.vector_index.upsert_with_boost(correction).await;
    }
}
```

---

## 2. Tier Specifications

### 2.1 Tier 0: Merchant Lookup

**Data Sources:**
- `greggles/mcc-codes` — 800+ MCC codes in JSON
- `seed_merchants.json` — curated list of ~500 common merchants
- Plaid PFC taxonomy — category hierarchy reference
- Runtime learned — auto-populated from Tier 3 and user corrections

**Algorithm:**
```
1. Normalize description: lowercase, strip numbers/punctuation, collapse whitespace
2. Exact match against merchant_registry.exact_map → O(1)
3. If no exact match: extract prefix (first 3 chars normalized)
   → fuzzy match against prefix_index candidates using Jaro-Winkler
   → accept if similarity ≥ 0.88
4. If MCC code available: lookup mcc_map → direct category mapping
```

**Rust Implementation:**
```rust
pub trait MerchantLookup: Send + Sync {
    fn lookup(&self, description: &str, mcc: Option<u16>) -> Option<CategoryAssignment>;
}
```

**Dependencies:** `strsim` crate (Jaro-Winkler), `serde_json` (load MCC data)

### 2.2 Tier 1: Pattern Engine

**Algorithm:**
```
1. Compile all patterns into a RegexSet (evaluated in single pass)
2. For each unmatched transaction, test against RegexSet
3. First matching pattern wins (priority-ordered)
4. Amount-range heuristics for ambiguous matches:
   - Positive amounts > $500 → likely "income"
   - Description contains "PAYROLL" → income/salary
   - Round amounts ($X00) to insurance/utility companies → bills
```

**Rust Implementation:**
```rust
pub trait PatternMatcher: Send + Sync {
    fn match_pattern(&self, description: &str, amount: Decimal) -> Option<CategoryAssignment>;
}
```

**Dependencies:** `regex` crate (RegexSet for multi-pattern single-pass matching)

### 2.3 Tier 2: RuVector Semantic Search

**Architecture:**
```
Transaction Description
    │
    ▼
[ONNX Embedding Model]     ← RuVector built-in, <10ms
    │
    ▼
[HNSW Index Query]          ← 61µs p50, 80K QPS
    │
    ▼
[Top-K neighbors]           ← k=5, weighted vote
    │
    ▼
[SONA Adaptation]           ← LoRA micro-update + EWC++ memory
    │
    ▼
CategoryAssignment (if confidence ≥ 0.85)
```

**Bootstrap Process:**
1. Take all categorized transactions from DB (initially from LLM Tier 3)
2. Embed each description using RuVector's ONNX model
3. Insert into HNSW index with metadata: `{category, subcategory, confidence}`
4. On query: embed new description → find 5 nearest neighbors → weighted majority vote
5. SONA automatically adjusts weights based on which neighbors produce correct predictions

**Rust Implementation:**
```rust
pub trait SemanticCategorizer: Send + Sync {
    async fn categorize(&self, description: &str) -> Option<CategoryAssignment>;
    async fn learn(&self, description: &str, assignment: &CategoryAssignment);
}
```

**GNN Enhancement:**
RuVector's GNN layer models relationships beyond simple embedding similarity:
- Merchant → Category edges (weighted by frequency)
- Amount-range → Subcategory edges (e.g., Starbucks $5 = coffee, $45 = catering)
- Temporal patterns (e.g., monthly recurring = bill, weekly = groceries)
- Co-occurrence (transactions at same merchant often share subcategory)

### 2.4 Tier 3: LLM Batch Inference (Optimized)

**Key changes from current implementation:**

| Aspect | Current | Optimized |
|--------|---------|-----------|
| Protocol | Tool-calling (1 call per txn) | Batch JSON array |
| Parallelism | Sequential batches | `OLLAMA_NUM_PARALLEL=4` |
| Context window | 262K (default) | 4096 tokens |
| Output mode | Free-form | `format: "json"` constrained |
| Thinking | Model-dependent | `think: false` |
| Batch size | 25 txns | 50 txns (with smaller context) |
| Transactions reaching T3 | 100% | 3-8% |

**Batch JSON Protocol:**
```json
// System prompt: "Return a JSON array of categorizations..."
// User prompt: lists 50 transactions
// Response (constrained by format: "json"):
[
  {"idx": 1, "cat": "food_dining", "sub": "groceries", "merchant": "Whole Foods", "conf": 0.95},
  {"idx": 2, "cat": "transportation", "sub": "gas_fuel", "merchant": "Shell", "conf": 0.92},
  ...
]
```

**Throughput math (optimized):**
- 50 txns per request, ~3-5s per request (with small model, no thinking)
- 4 parallel requests = 200 txns / 5s = 40 txn/s
- 500 remaining transactions = 12.5 seconds

---

## 3. Self-Learning Feedback Loop

```
                    ┌─────────────────────┐
                    │   USER CORRECTION   │
                    │  (payee rule, edit) │
                    └──────────┬──────────┘
                               │
                               ▼
┌──────────┐          ┌────────────────┐           ┌──────────┐
│  Tier 3  │──────────│   FEEDBACK     │───────────│  Tier 0  │
│  LLM     │  result  │   PROCESSOR    │  merchant │ Merchant │
│  Output  │──────────│                │───────────│ Registry │
└──────────┘          │  Routes events │           └──────────┘
                      │  to all tiers  │
                      │                │          ┌──────────┐
                      │                │──────────│  Tier 2  │
                      │                │ embedding│ RuVector │
                      │                │──────────│ HNSW     │
                      └────────────────┘          └──────────┘
                               │
                               ▼
                      ┌────────────────┐
                      │  SONA Engine   │
                      │  • LoRA adapt  │
                      │  • EWC++ guard │
                      │  • GNN update  │
                      └────────────────┘
```

**Learning triggers:**
1. **Every LLM result** → Add embedding to Tier 2 index. If confidence ≥ 0.9, promote merchant to Tier 0.
2. **Every user correction** → Update Tier 0 merchant entry (highest priority). Update Tier 2 embedding with boosted weight. Add regex pattern to Tier 1 if description is pattern-like.
3. **Every Tier 2 query** → SONA micro-updates LoRA weights. EWC++ prevents forgetting previously learned patterns.
4. **Weekly batch** → Validate Tier 0 merchants against recent LLM labels. Prune stale entries not seen in 90 days.

**Cold-start bootstrap:**
1. Minute 0: Load MCC codes + seed merchants → Tier 0 works immediately
2. Minute 1: Load existing payee rules → Tier 1 works immediately
3. Minutes 2-10: First 10K transactions flow through. Tier 3 handles 100%. Results feed back to Tiers 0 and 2.
4. Next batch: Tier 0 handles ~50%. Tier 2 handles ~20%. Tier 3 drops to ~30%.
5. After 50K transactions: Tier 0 handles ~70%. Tier 2 handles ~15%. Tier 3 handles ~5%.

---

## 4. Database Schema Changes

### 4.1 New columns on `transactions`

```sql
-- Migration: 017_categorization_tier.sql
ALTER TABLE transactions
  ADD COLUMN source_tier TEXT DEFAULT 'llm';

-- Track which tier assigned the category for observability.
-- Values: 'merchant_lookup', 'pattern_engine', 'semantic_search', 'llm', 'user'
```

### 4.2 New table: `merchant_registry`

```sql
-- Migration: 018_merchant_registry.sql
CREATE TABLE merchant_registry (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    canonical_name TEXT NOT NULL,
    aliases TEXT[] NOT NULL DEFAULT '{}',
    category TEXT NOT NULL,
    subcategory TEXT NOT NULL DEFAULT '',
    confidence DOUBLE PRECISION NOT NULL DEFAULT 1.0,
    source TEXT NOT NULL,  -- 'mcc', 'seed', 'llm_learned', 'user_defined'
    hit_count BIGINT NOT NULL DEFAULT 0,
    last_seen TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(canonical_name)
);

CREATE INDEX idx_merchant_registry_aliases ON merchant_registry USING GIN (aliases);
CREATE INDEX idx_merchant_registry_category ON merchant_registry (category);
```

---

## 5. Implementation Phases

### Phase 1: LLM Optimization (3 days)

**No new crate.** Optimize existing `finima-llm` for maximum LLM throughput.

| Task | File | Change |
|------|------|--------|
| Batch JSON protocol | `finima-llm/src/prompts.rs` | New prompt returning JSON array instead of tool calls |
| JSON output parser | `finima-llm/src/batch_json.rs` | Parse `[{idx, cat, sub, conf}]` array |
| Parallel requests | `finima-llm/src/client.rs` | Send N concurrent batch requests via `tokio::JoinSet` |
| Reduced context | `finima-llm/src/client.rs` | Add `num_ctx: 4096` to Ollama request body |
| Config | `config/llm.yaml` | Add `parallel_requests: 4`, `num_ctx: 4096` |

**Validation:** Categorize 706 transactions. Measure total time. Target: under 5 minutes.

### Phase 2: Merchant Lookup (5 days)

**New crate:** `finima-categorize` with Tier 0 only.

| Task | File | Change |
|------|------|--------|
| Create crate | `crates/finima-categorize/` | Cargo.toml, lib.rs |
| MCC loader | `tier0/mcc_loader.rs` | Parse `greggles/mcc-codes` JSON |
| Merchant DB | `tier0/merchant_db.rs` | HashMap + prefix index + fuzzy match |
| Seed data | `data/seed_merchants.json` | 500 common merchants with categories |
| DB migration | `finima-db/migrations/018_merchant_registry.sql` | Persistent merchant storage |
| Integration | `finima-api/handlers/categorization.rs` | Try Tier 0 before LLM |

**Validation:** Import 10K transactions. Verify Tier 0 resolves 60%+ without LLM. Total time under 2 minutes.

### Phase 3: RuVector Integration (5 days)

**Add Tier 2 to `finima-categorize`.**

| Task | File | Change |
|------|------|--------|
| RuVector dep | `finima-categorize/Cargo.toml` | Add `ruvector` dependency |
| Embedder | `tier2/embedder.rs` | ONNX text embedding via RuVector |
| HNSW index | `tier2/ruvector_backend.rs` | Build/query HNSW with category metadata |
| SONA config | `tier2/ruvector_backend.rs` | Enable self-learning on queries |
| Bootstrap | `tier2/mod.rs` | Seed index from existing categorized transactions |
| Integration | `engine.rs` | Wire Tier 2 between Tier 1 and Tier 3 |

**Validation:** Re-categorize 10K transactions. Verify Tier 2 handles 10-15% of previously-LLM-only transactions. HNSW query latency < 1ms p99.

### Phase 4: Feedback Loop & Patterns (5 days)

**Complete the self-learning cycle.**

| Task | File | Change |
|------|------|--------|
| Feedback processor | `feedback/mod.rs` | Route events to all tiers |
| LLM → Tier 0 | `feedback/merchant_learner.rs` | Auto-promote high-confidence merchants |
| LLM → Tier 2 | `feedback/vector_learner.rs` | Add embeddings to HNSW index |
| User → all tiers | `feedback/user_correction.rs` | Override propagation |
| Pattern engine | `tier1/regex_engine.rs` | RegexSet-based pattern matching |
| Source tracking | Migration 017 | `source_tier` column on transactions |
| Dashboard | Frontend | Tier distribution chart |

**Validation:** Run 3 consecutive categorization batches of 10K. Verify Tier 3 (LLM) percentage decreases with each batch. System should reach <8% LLM dependency by batch 3.

---

## 6. Observability & Metrics

| Metric | Source | Alert Threshold |
|--------|--------|-----------------|
| `categorize_total_duration_ms` | CategorizationJob | > 600,000ms (10 min) |
| `categorize_tier_distribution` | TierStats | Tier 3 > 20% after 50K txns |
| `categorize_tier_latency_p99` | Per-tier timing | Tier 0 > 10ms, Tier 2 > 50ms |
| `merchant_registry_size` | MerchantRegistry | < 100 after 10K txns |
| `ruvector_index_size` | EmbeddingIndex | Unexpectedly low growth |
| `llm_batch_error_rate` | Tier 3 | > 10% failures |
| `sona_adaptation_count` | RuVector SONA | 0 (learning stopped) |
| `user_override_rate` | Corrections | > 15% (tiers are inaccurate) |

---

## 7. Risk Mitigation

| Risk | Impact | Mitigation |
|------|--------|------------|
| RuVector API instability | Tier 2 unavailable | Feature-flag Tier 2; degrade to Tier 1 → Tier 3 |
| SONA drift | Decreasing accuracy | EWC++ memory preservation; weekly validation against LLM labels |
| Merchant DB bloat | Memory pressure | Cap at 50K entries; LRU eviction on `last_seen` |
| MCC data staleness | Wrong categories | Community-sourced; auto-validate against LLM quarterly |
| Batch JSON parsing | LLM returns malformed JSON | `format: "json"` constrains output; fallback to tool-calling |
| Cold start on new deploy | No learned data | Merchant DB persisted in PostgreSQL; HNSW index serialized to disk |
