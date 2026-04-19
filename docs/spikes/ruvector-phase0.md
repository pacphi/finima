# Spike: RuVector Phase 0 — API Validation

**Branch:** `spike/ruvector-phase0`
**Date:** 2026-04-17
**Status:** Complete — proceed to Phase 1

## Goal

Validate that `ruvector-core` is usable as the Tier 2 backend for categorization
(ADR-012) and the SONA pattern matcher for flow detection (ADR-017), before
committing to a workspace dependency.

## What was built

Standalone scratch crate at `/tmp/ruvector-spike/` (not in the finima workspace).
A tiny program creates an in-memory `VectorDB`, inserts 10 labeled transaction
descriptions with a deterministic non-semantic embedding, and runs k-NN queries.

```toml
# Cargo.toml
ruvector-core = { version = "2.1", default-features = false,
                  features = ["hnsw", "simd", "memory-only"] }
```

## Build metrics (release, M-series mac)

| Metric                               | Value                                                                                                                                                    |
| ------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `ruvector-core` version on crates.io | **2.1.0** (latest)                                                                                                                                       |
| Clean build, wall clock              | **~17 s**                                                                                                                                                |
| CPU time                             | ~82 s user (parallel)                                                                                                                                    |
| Transitive deps                      | 120                                                                                                                                                      |
| Release binary size                  | **936 KB**                                                                                                                                               |
| Direct deps of ruvector-core         | 17 crates (hnsw_rs, simsimd, ndarray, rkyv, dashmap, parking_lot, rand, uuid, chrono, serde, bincode, thiserror, anyhow, tracing, rand_distr, once_cell) |

No C/system deps required at this feature set. No ONNX, no reqwest, no redb.

## Actual 2.1 API (what ADR-017 got wrong)

ADR-017 assumed `ruvector::HnswIndex` and `ruvector::SonaAdapter`. Neither name
exists. The real surface (from `ruvector-core` 2.1.0):

```rust
use ruvector_core::{VectorDB, VectorEntry, SearchQuery, SearchResult,
                    DistanceMetric, VectorId};
use ruvector_core::types::{DbOptions, HnswConfig, QuantizationConfig};

// IDs are Strings, not u64.
pub type VectorId = String;

// Entry is a struct with optional id + optional metadata (serde_json::Value map).
pub struct VectorEntry {
    pub id: Option<VectorId>,
    pub vector: Vec<f32>,
    pub metadata: Option<HashMap<String, serde_json::Value>>,
}

// Query goes in by value. Results carry `score` (not `distance`).
let hits: Vec<SearchResult> = db.search(SearchQuery { vector, k, filter: None, ef_search: None })?;
// SearchResult { id, score, vector: Option<..>, metadata: Option<..> }
```

`VectorDB::new(DbOptions)` is the canonical constructor. `with_dimensions` is
a shortcut when defaults are fine. `insert` / `insert_batch` return the
assigned `VectorId`.

## SONA — not where we thought

There is **no `SonaAdapter` in `ruvector-core`**, and the crates.io crate named
`sona` is an unrelated 2022 placeholder (0.0.0, unowned by ruvnet). Self-learning
lives outside `ruvector-core` in the in-repo `sona` / `ruvector-cognitive-*`
crates, which are **not published to crates.io**. Options for us:

1. Skip SONA self-learning for now; use raw HNSW + explicit feedback loop
   (insert confirmed patterns, decay dismissed ones in application code).
   This is enough to satisfy ADR-012 Tier 2 and most of ADR-017.
2. Git-dep on `ruvnet/ruvector` at a pinned commit for SONA. Adds supply-chain
   risk.
3. Wait for upstream to publish SONA.

**Recommendation: option 1.** Ship with a pluggable trait boundary, treat SONA
as a later swap-in.

## Embeddings

`ruvector-core` does not generate embeddings at the `hnsw,simd,memory-only`
feature set — it only stores and searches them. Options offered by the crate:

| Feature                                        | What it pulls in                     | Use                        |
| ---------------------------------------------- | ------------------------------------ | -------------------------- |
| `hash-embeddings` (default of `HashEmbedding`) | none                                 | testing only, NOT semantic |
| `api-embeddings`                               | reqwest (+rustls)                    | OpenAI-compatible HTTP     |
| `onnx-embeddings`                              | `ort` 2.0-rc, `tokenizers`, `hf-hub` | local semantic, big deps   |
| `real-embeddings`                              | Candle                               | local semantic, also large |

For the Tier 2 MVP we can start with a small sentence-transformer via
`onnx-embeddings` OR keep our own ONNX wiring and just hand ruvector the
vectors. Deferred to Phase 1 decision.

## Risks surfaced

1. **ADR-017 version pin is stale** (`0.1` → actually `2.1`). Fixed in this
   spike — see ADR edit below.
2. **API names in ADR-017 are wrong** (`HnswIndex`, `SonaAdapter`). Will be
   updated during Phase 1 implementation, not pre-emptively.
3. **SONA is not on crates.io.** Capability downgrade: we get HNSW + quantization
   - hybrid search, we do not get auto-tuning / LoRA / EWC++ without a git dep.
4. **ONNX runtime is heavy.** If we enable `onnx-embeddings` the dep graph
   grows significantly (ort 2.0-rc is pre-1.0). Keep embedding generation
   behind its own feature flag, separate from the vector-store feature flag.
5. **`rkyv` 0.8** and **`bincode` 2** are both in the transitive graph —
   validate these don't collide with any pinned versions elsewhere when we
   add to the real workspace.

## Verdict

Green light for Phase 1. The crate is functional, small at the feature set we
need, and the API is close enough to the shape our traits (`SemanticCategorizer`,
`FlowPatternMatcher`) already assume. The main delta from ADR-017 is that we
do the self-learning bookkeeping ourselves rather than relying on a SONA
adapter.

## Follow-ups

- [x] Update ADR-017 version pin to `2.1`.
- [x] Phase 0b: validate `ruvector-sona`. See [Phase 0b addendum](#phase-0b-addendum--ruvector-sona).
- [x] Phase 0c: persistence round-trip. See [Phase 0c addendum](#phase-0c-addendum--persistence-round-trip).
- [x] Phase 0d: MicroLoRA training-path verification. See [Phase 0d addendum](#phase-0d-addendum--what-actually-makes-microlora-adapt).
- [ ] Phase 1: implement `RuVectorEmbeddingStore: SemanticCategorizer` in
      `crates/finima-categorize/src/tier2/ruvector_store.rs` behind a `sona`
      feature.
- [ ] Phase 1: decide embedder (ONNX local vs. small Rust-native model vs.
      bring-your-own).
- [ ] Phase 2: implement `RuVectorPatternMatcher: FlowPatternMatcher` backed
      by `ruvector-core::VectorDB` (HNSW) and `ruvector-sona::SonaEngine`
      (feedback loop), and add `flow_patterns` migration.

---

## Phase 0b addendum — ruvector-sona

**Correction to earlier note:** `ruvector-sona` **is** on crates.io; the Phase 0
finding that SONA was unpublished was wrong — I had searched for the crate
name `sona` (an unrelated 2022 placeholder). The published package name is
`ruvector-sona`.

**Latest:** `ruvector-sona = "0.1.9"` (published 2026-03-25, 6.4K downloads,
MIT/Apache-2.0).

**Scratch binary:** `/tmp/ruvector-spike/src/sona_spike.rs` — builds against
`ruvector-sona = "0.1"`, feeds 25 trajectories (20 positive, 5 dismissed),
forces learning, measures micro-LoRA latency and pattern recall.

### Dep footprint

- Added crates beyond Phase 0: only `parking_lot` / `crossbeam` / `rand` were
  already present transitively — **net new build graph is effectively zero**.
- Incremental rebuild after adding `ruvector-sona`: ~8 s (crate is small).
- No ONNX, no C deps, no workspace coupling.

### Real API (0.1.9) vs. README

README examples drift from reality in several places. Canonical usage:

```rust
use ruvector_sona::engine::SonaEngineBuilder;   // not re-exported at root
use ruvector_sona::SonaEngine;

let engine: SonaEngine = SonaEngineBuilder::new()
    .hidden_dim(256)
    .micro_lora_rank(2)
    .base_lora_rank(8)
    .build();

// trajectory lifecycle
let mut tb = engine.begin_trajectory(query_embedding);  // -> TrajectoryBuilder
tb.add_step(activations, attention_weights, reward);    // owned Vecs
engine.end_trajectory(tb, quality);                     // consumes builder

// inference adaptation — caller-owned output buffer (not a return value)
let mut out = vec![0.0f32; 256];
engine.apply_micro_lora(&input, &mut out);

// pattern retrieval + stats
let hits: Vec<LearnedPattern> = engine.find_patterns(&query, k);
let s: CoordinatorStats = engine.stats();
```

### Measured performance (Apple Silicon release build)

| Operation                                          | Value                        |
| -------------------------------------------------- | ---------------------------- |
| `begin_trajectory` + `add_step` + `end_trajectory` | **3.2 µs** / trajectory      |
| `apply_micro_lora` p50 / p95 / p99 (n=1000)        | **1.2 / 1.3 / 1.3 µs**       |
| `force_learn` on 25 trajectories                   | synchronous, immediate       |
| `find_patterns(k=3)` after 25 trajectories         | 3 hits returned              |
| Mean per-dim abs-delta after adaptation            | 0.055 (non-trivial mutation) |

All comfortably inside our Tier 2 latency budget (<10 ms). The <1 ms
adaptation claim in upstream marketing holds up for `hidden_dim=256`.

### Quirks / open questions

1. **`CoordinatorStats.trajectories_recorded` stayed at 0** even after 25
   `end_trajectory` calls, while `patterns_stored` / `patterns_learned`
   correctly reached 25. The `trajectories_*` counters likely track the
   async ring-buffer path (`submit_trajectory`) rather than the synchronous
   `end_trajectory` path. Need to confirm when we wire the feedback loop —
   if we want those counters to move, we may need to flip to
   `submit_trajectory(QueryTrajectory)` instead.

2. **README examples are aspirational.** Code against docs.rs + source, not
   the README. `SonaEngine::builder()` (root method) does not exist; the
   builder lives in the `engine` module. `apply_micro_lora` writes into an
   out-buffer, not a return value. `stats()` not `get_stats()`.

3. **No persistence built in.** SONA keeps state in-process. We'll need to
   serialize patterns / LoRA state ourselves if we want to survive restarts
   — there is `export_lora_state()` returning a safetensors `LoRAState`
   which gives us a path. Worth one more 30-minute spike during Phase 1 to
   validate round-tripping.

### Verdict

Green-light proceeding with `ruvector-sona` as the self-learning layer on
top of `ruvector-core`'s HNSW. Submodule approach (discussed earlier) is
unnecessary and explicitly not recommended: it adds a 545 MB clone and
workspace-root collisions for no capability gain.

---

## Phase 0c addendum — persistence round-trip

**Goal:** answer "what actually survives a process restart?" before we build
cold-start behavior into the real code. Three parts run against
`ruvector-sona` 0.1.9 and `ruvector-core` 2.1 (memory-only).

**Scratch binary:** `/tmp/ruvector-spike/src/persist_spike.rs`.

### A. SONA ReasoningBank patterns — ✅ full round-trip

```rust
let json: String = engine_a.coordinator().serialize_state();
// ...later, or in a new process...
let engine_b: SonaEngine = SonaEngineBuilder::new().hidden_dim(256).build();
engine_b.coordinator().load_state(&json)?;   // returns Ok(n_patterns)
```

| Metric                      | Value                              |
| --------------------------- | ---------------------------------- |
| 50 patterns → JSON size     | **274 KB**                         |
| `serialize_state()` latency | **0.7 ms**                         |
| `load_state()` latency      | **0.6 ms**                         |
| `find_patterns` equivalence | **bit-identical** before vs. after |

The JSON is self-describing (includes `version`, `patterns`, `ewc_task_count`,
loop-enabled flags). Safe to store in a Postgres `TEXT`/`JSONB` column per
portfolio. Stripping whitespace / gzipping would cut it significantly but 274 KB
for 50 patterns is already fine.

### B. SONA LoRA weights — ❌ one-way export only

```rust
let state: LoRAState = engine.export_lora_state();      // weights as Vec<f32>
HuggingFaceExporter::with_config(&engine, cfg)
    .export_lora_safetensors(&path)?;                    // writes adapter_model.safetensors
```

| Metric                            | Value                |
| --------------------------------- | -------------------- |
| micro_lora_layers                 | 1 (rank 2, 256×256)  |
| base_lora_layers                  | 12 (rank 8, 256×256) |
| `export_lora_safetensors` latency | 0.8 ms               |
| safetensors file size             | **784 KB**           |

Grepped the 0.1.9 source for any `import`/`load`/`restore`/`from_safetensors`
hook into a `SonaEngine` — there is none. The exporter is fire-and-forget for
external HF fine-tuning consumers, not a save-and-reload mechanism.

**Consequence:** LoRA adapter weights do NOT survive a process restart. A
fresh `SonaEngine` constructed with the same builder produces identical
`apply_micro_lora` output to a "trained" engine in this spike — mean |Δ| per
dim was 0.054628 in both. This also flags a secondary concern worth a future
0d spike: it's unclear whether the synchronous `end_trajectory` → `force_learn`
path actually mutates LoRA weights in 0.1.9, or whether micro-LoRA updates
require pumping the `InstantLoop` explicitly. For Phase 1 we sidestep this by
relying on ReasoningBank (which definitely learns and persists) as the primary
signal.

### C. `VectorDB` memory-only snapshot — ✅ via application-level dump

`ruvector-core`'s redb persistence needs the `storage` feature (adds redb +
memmap2 deps). At `memory-only` we dump via the public API:

```rust
let keys = db.keys()?;
let snapshot: Vec<VectorEntry> = keys.iter()
    .filter_map(|k| db.get(k).transpose())
    .collect::<Result<_,_>>()?;
// persist `snapshot` (we use JSON below; see note)
```

| Metric                             | Value                     |
| ---------------------------------- | ------------------------- |
| 10 entries (dim=64) → JSON size    | 8.5 KB                    |
| Dump latency                       | < 0.1 ms                  |
| Reload into fresh VectorDB         | 0.1 ms                    |
| Ranked search ids, before vs after | **identical**             |
| Search scores, before vs after     | **identical within 1e-5** |

**Gotcha:** `bincode = "1"` does not work — `VectorEntry.metadata` uses
`serde_json::Value` which invokes `deserialize_any`, unsupported by non-self-
describing formats. Viable production choices, in order of preference:

1. Store entries in Postgres: `id TEXT PK`, `embedding VECTOR(D)` (pgvector)
   or `BYTEA` (raw f32), `metadata JSONB`. Rebuild in-memory `VectorDB` on
   boot. Avoids a second storage backend; integrates with existing migrations.
2. `rmp-serde` (MessagePack) — self-describing, compact, small dep.
3. Enable `ruvector-core/storage` feature (redb). Adds redb + memmap2 to build
   graph; introduces a second persistence layer alongside Postgres.

Recommendation: **option 1**. It matches ADR-012's `EmbeddingIndex` /
`flow_patterns` table designs.

### Phase 0c verdict

| Concern                                   | Resolution                                       |
| ----------------------------------------- | ------------------------------------------------ |
| Reasoning patterns survive restart        | ✅ JSON round-trip, bit-identical retrieval      |
| LoRA weights survive restart              | ❌ No import API — accept that LoRA re-converges |
| Vector index survives restart             | ✅ Dump via `keys()`/`get()`, reinsert on boot   |
| Can we skip ruvector's `storage` feature? | ✅ Yes — we have Postgres                        |

Cold-start design for Phase 1:

```text
startup
  ├── read flow_patterns / embedding_index rows from Postgres
  ├── build_engine = SonaEngineBuilder::new()...build()
  ├── build_engine.coordinator().load_state(saved_json)   // retrieval
  └── foreach row: vector_db.insert(...)                   // HNSW

periodic (e.g., every N confirmations or every hour)
  ├── serialize_state() -> upsert into portfolios.sona_state
  └── dump new vectors -> insert missing into embedding_index
```

No LoRA snapshot needed; LoRA will re-warm as soon as trajectories start
recording again post-restart.

### Follow-ups added by this spike

- [x] Phase 0d: verify whether `apply_micro_lora` output actually shifts
      after training. **Answered below — it does, but only under specific
      conditions we had not satisfied.**
- [ ] Phase 1 schema: `portfolios.sona_state JSONB NULL` column, plus the
      existing `embedding_index` / `flow_patterns` tables from ADR-012 /
      ADR-017.
- [ ] Phase 1 policy: persist SONA state on N confirmations OR M minutes,
      whichever first.

---

## Phase 0d addendum — what actually makes MicroLoRA adapt

**Motivation:** in 0c, fresh and "trained" engines produced bit-identical
`apply_micro_lora` output, raising the possibility that SONA's instant
adaptation claim doesn't apply to our usage pattern.

**Scratch binary:** `/tmp/ruvector-spike/src/lora_spike.rs`.

### Source-code reading first

From `ruvector-sona-0.1.9/src/loops/instant.rs` and `lora.rs`:

1. `end_trajectory()` → `coordinator.on_inference()` → `instant.on_trajectory()`.
2. `on_trajectory` calls `micro_lora.accumulate_gradient(&signal)` and bumps
   a pending-signal counter. **Gradients are applied only when
   `pending ≥ flush_threshold` (default 100)**, OR on `engine.flush()`.
3. `force_learn()` runs the _background_ cycle (ReasoningBank + BaseLoRA +
   EWC). It does **not** flush MicroLoRA.
4. `MicroLoRA::new` initialises with the standard LoRA scheme: `A` random,
   `B = 0`. So `forward(x)` returns zero until `B` is updated.
5. The learning signal for one trajectory is computed by
   `LearningSignal::estimate_gradient` as REINFORCE:
   `grad = Σ (reward_i − baseline) × activation_i` across steps, then
   L2-normalized.
6. With a **single-step trajectory**, reward == baseline, so advantage == 0,
   so gradient == 0. Accumulating zero gradients forever ⇒ `B` stays zero
   ⇒ `apply_micro_lora` keeps returning the zero vector.

### Empirical verification

All five scenarios below compare `apply_micro_lora(probe)` against a fresh
engine's output on the same probe (single-step trajectories throughout):

| Scenario (all single-step, rank 2, hidden=256) | L2 Δ         | patterns |
| ---------------------------------------------- | ------------ | -------- |
| A. 25 traj + `force_learn`                     | **0.000000** | 25       |
| B. 25 traj + `engine.flush()` only             | **0.000000** | 0        |
| C. 99 traj (below auto-flush, no flush)        | **0.000000** | 0        |
| D. 100 traj (hits auto-flush threshold)        | **0.000000** | 0        |
| E. 500 traj + flush + `force_learn`            | **0.000000** | 100      |
| F. 100 traj via `submit_trajectory()` + flush  | **0.000000** | 0        |

All zeros. Matches the theoretical prediction.

Then the multi-step control:

| Scenario                                            | L2 Δ     |
| --------------------------------------------------- | -------- |
| G. 200 × **3-step trajectories** + `engine.flush()` | **1e-6** |

Small but nonzero — MicroLoRA did mutate. Magnitude is constrained by the
default `micro_lora_lr = 0.002` and the normalized gradient; it would rise
with more diverse trajectories and tuned learning rate.

### What this means for our design

1. **ReasoningBank is the load-bearing mechanism**, not MicroLoRA. It works
   out of the box, it persists (0c), and retrieval quality is what the
   ADR-017 user stories actually depend on.
2. **MicroLoRA learning requires multi-step trajectories with reward
   variance across steps.** For flow-pattern matching, that would mean
   recording a trajectory per detection pass with intermediate "steps"
   (e.g., heuristic match attempt, k-NN retrieval, final decision) each
   tagged with its own confidence as the `reward`. This is implementable,
   but it is _extra_ instrumentation we had not planned.
3. **`force_learn()` ≠ flush.** We were wrong in 0b to treat them as
   equivalent. Production code must call `engine.flush()` at the end of a
   detection pass (or rely on the 100-signal auto-flush) if we want
   MicroLoRA updates applied.
4. The marketing line "SONA adapts in <1 ms per query" is narrowly true
   (`apply_micro_lora` forward is ~1.2 µs p50) but the _learning_ is
   batched at flush time, not per-query. No change to our latency budget.
5. **LoRA adaptation is best-effort for MVP.** Given that LoRA weights
   don't survive restart (0c) and require bespoke instrumentation to
   train (this spike), the honest plan is: build Phase 1 with
   ReasoningBank-only, add MicroLoRA as a Phase 2/3 enhancement once we
   have real usage and real intermediate reasoning signals to record.

### Updated integration sketch

```rust
// pass boundary
for pattern in hnsw.search(&query_embedding, k) {
    // record a multi-step trajectory for future LoRA training
}
engine.flush();   // trigger batch apply if any gradients accumulated

// on feedback (confirm/dismiss)
let mut tb = engine.begin_trajectory(query_embedding);
tb.add_step(retrieval_activations, attention_weights, retrieval_confidence);
tb.add_step(final_activations, final_attention, final_confidence);
engine.end_trajectory(tb, user_quality);
```

### Phase 0d verdict

- ADR-017's "SONA adapts LoRA weights on each query cycle" is **aspirational
  as written.** Real behavior: patterns learn immediately and persist;
  LoRA learns in batches and doesn't persist. Neither is a blocker.
- No new spikes needed. **Ready to land Phase 0 docs and start Phase 1.**
