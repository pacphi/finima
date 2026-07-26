# Embedder Configuration

Finima's Tier 2 semantic categorization and the ADR-017 flow-pattern
matcher both consume vectors from a pluggable embedder. This guide
covers selection, configuration, and operational tuning.

## Backends

| Backend  | Use case                                        | Deps                               | Setup                              |
| -------- | ----------------------------------------------- | ---------------------------------- | ---------------------------------- |
| `none`   | CI / tests / BYO vectors                        | none                               | always available                   |
| `ollama` | Local HTTP — reuses the existing Ollama service | `reqwest`                          | run `ollama pull nomic-embed-text` |
| `candle` | Local in-process BertModel                      | `candle-*`, `hf-hub`, `tokenizers` | first run downloads the model      |

No external / paid providers are supported (OpenAI, Cohere, etc.)
by design.

## Selection

Two knobs:

1. **Compile time** — the Cargo feature flag on `finima-api`:
   `embedder-ollama`, `embedder-candle`, `embedder-candle-metal`,
   `embedder-candle-cuda`. Missing the feature means that backend
   value in YAML silently falls back to `none` with a warning
   logged at startup.
2. **Runtime** — the `embedder.backend` field in
   `config/categorize.yaml` (or `APP__EMBEDDER__BACKEND` env var).

The Makefile `EMBEDDER=` flag wires both at once. It inherits from
`LLM=` by default, so `make start LLM=ollama` also enables the
Ollama embedder:

```bash
make start LLM=ollama                       # Ollama LLM + Ollama embedder
make start LLM=ollama EMBEDDER=none         # Ollama LLM, no embedder
make start LLM=none EMBEDDER=candle         # no LLM, local Candle embedder
make start LLM=none EMBEDDER=candle-metal   # Apple Silicon acceleration
```

## YAML layout (`config/categorize.yaml`)

```yaml
categorize:
  # ... tier 0/1 settings ...
  tier2:
    backend: jaccard # jaccard | ruvector
    dim: 384 # must match embedder.dim for ruvector
    hnsw_m: 32
    hnsw_ef_construction: 200
    hnsw_ef_search: 100
    bootstrap_on_start: true
    bootstrap_max_examples: 0 # 0 = unbounded

embedder:
  backend: none # none | ollama | candle
  dim: 384 # must match categorize.tier2.dim for RuVector
  ollama:
    url: http://localhost:11434
    model: nomic-embed-text
    timeout_millis: 30000
  candle:
    model_id: sentence-transformers/all-MiniLM-L6-v2
```

## Dimension matching

`embedder.dim` MUST equal `categorize.tier2.dim` when the RuVector
backend is active. A mismatch causes insertion rejects (counted as
`tier2_bootstrap_rejected_total`) and `DimMismatch` at query time.
Recommended pairings:

- MiniLM-L6-v2 (Candle) → dim=384
- nomic-embed-text (Ollama) → dim=768
- mxbai-embed-large (Ollama) → dim=1024

Set `embedder.dim` and `categorize.tier2.dim` consistently. If you
change models later, regenerate the Tier 2 index via the
bootstrap bin.

## Bootstrap

After changing the embedder or the Tier 2 backend, repopulate the
index:

```bash
cargo run --bin bootstrap_tier2 -- --portfolio-id $PID
cargo run --bin bootstrap_flows -- --portfolio-id $PID
```

Flags: `--dry-run`, `--limit N`. Both bins log structured
`offered / inserted / rejected / elapsed_ms` summaries.

## Ollama prerequisites

```bash
ollama pull nomic-embed-text
# verify:
curl -s http://localhost:11434/api/embeddings \
  -d '{"model":"nomic-embed-text","prompt":"hello"}' | jq '.embedding | length'
# expect: 768
```

## Candle prerequisites

First call to `embed()` downloads the model (~90 MB for MiniLM) to
`~/.cache/huggingface/hub/`. Subsequent runs are offline. Override
the cache with `HF_HOME=/path/to/cache`.

For Apple Silicon acceleration use `EMBEDDER=candle-metal`; for
NVIDIA, `EMBEDDER=candle-cuda`. Plain `EMBEDDER=candle` auto-promotes
to the best available accelerator detected by the Makefile (mirrors
`LLM=candle`).

## Metrics

Request-time counters and histograms surface under `/metrics`:

- `tier2_queries_total{backend, outcome}`
- `tier2_search_latency_seconds{backend}`
- `flow_pattern_queries_total{backend, outcome}`
- `flow_pattern_search_latency_seconds{backend}`
- `tier2_bootstrap_{inserted,rejected}_total`
- `flow_patterns_{confirmed,dismissed}_total`
- `tier2_index_size`, `flow_pattern_index_size` (gauges)
- `bootstrap_duration_seconds{component, result}` (histogram)

Bootstrap bins emit tracing logs only; they do not mutate the API
process's metric registry.

## Staging rollout runbook (issue #31)

Enabling `categorize.tier2.backend=ruvector` (ADR-012) in a non-prod
environment for a live lift measurement against the Jaccard baseline.
`config/staging.yaml` provides the environment overlay; this section is
the operational procedure for using it.

### The footgun: two conditions must BOTH hold

`config/staging.yaml` sets `categorize.tier2.backend: ruvector`, but that
value only takes effect if **both** of these are true at once:

1. **`APP_ENV=staging`** is set on the running process, so the
   `config/{APP_ENV}.yaml` overlay mechanism (`load_config_from`,
   `crates/finima-api/src/config.rs`) actually loads
   `config/staging.yaml` on top of the section files.
2. **The binary was compiled with the `sona` Cargo feature**
   (`finima-api`'s `sona = ["finima-categorize/sona",
   "finima-analysis/sona"]`, `crates/finima-api/Cargo.toml:74`).

If either is missing — most likely (2), since it's easy to deploy a
binary built without `--features sona` while still setting
`APP_ENV=staging` — `Tier2Config::resolved_backend()`
(`crates/finima-categorize/src/config.rs`) **silently downgrades
`ruvector` back to `jaccard`** and only logs a `tracing::warn!` at
startup. The API starts normally, `/health` is green, requests succeed
— and Tier 2 is quietly still running Jaccard. There is no hard failure;
the only signal is that startup log line (or explicitly checking the
resolved backend), so treat step 1 below as mandatory, not optional.

### Steps

1. **Build with the `sona` feature enabled.** The Makefile's
   `build-release` target does not currently expose a `SONA=` flag (only
   `LLM=` / `EMBEDDER=` are wired to Cargo features), so add `sona`
   directly to the feature list. `config/staging.yaml` pairs
   `categorize.tier2.backend=ruvector` with `embedder.backend=candle`
   (see the comments in that file for why `candle` over `ollama`), which
   needs the `embedder-candle` feature:

   ```bash
   cargo build --release -p finima-api --features sona,embedder-candle
   ```

   Verify with `cargo run -p finima-api --features sona,embedder-candle -- --version`
   or just watch the startup logs in step 2 for the resolved backend.

2. **Deploy with `APP_ENV=staging`** so the `config/staging.yaml`
   overlay applies:

   ```bash
   APP_ENV=staging ./target/release/finima-api
   ```

   Confirm the startup logs report `backend=ruvector` (not a
   `resolved_backend` fallback warning) for Tier 2, and
   `backend=candle` for the embedder.

3. **Bootstrap the semantic stores** for each active staging portfolio.
   Both bootstrap bins exist and mirror each other (ADR-012 / ADR-017
   Phase 2.C):

   ```bash
   # Tier 2 semantic categorizer (ADR-012)
   cargo run -p finima-api --bin bootstrap_tier2 -- --portfolio-id <PORTFOLIO_ID>

   # Flow-pattern matcher (ADR-017)
   cargo run -p finima-api --bin bootstrap_flows -- --portfolio-id <PORTFOLIO_ID>
   ```

   Both support `--dry-run` and `--limit N`; both log
   `offered / inserted / rejected / elapsed_ms` summaries — check for
   `rejected` counts, which usually mean a dimension mismatch (see
   "Dimension matching" above).

4. **Observe metrics at `/metrics`** for `tier2_queries_total{backend,
   outcome}` and `tier2_search_latency_seconds{backend}`, comparing the
   `ruvector` series against the `jaccard` baseline from before the
   rollout. If issue #32's gauge wiring has landed
   (`AppState::set_metrics` in `crates/finima-api/src/state.rs`,
   `handlers/flows.rs`'s confirm branch), `tier2_index_size` and
   `flow_pattern_index_size` are also live and worth watching alongside
   the query/latency series to confirm the bootstrapped index is
   actually growing as staging traffic confirms categorizations. Issue
   #33's integration test
   (`crates/finima-api/tests/tier2_flow_persistence_test.rs`) covers the
   bootstrap → persist → query path end-to-end, which reduces (but does
   not eliminate) the risk of a silent data-path regression during this
   rollout.

5. **Follow-up (operational, not code) — NOT done by this change:**
   the issue's acceptance criteria call for observing the metrics in
   step 4 for roughly a week of live staging traffic, then writing a
   lift comparison (RuVector vs. the Jaccard baseline) under
   `docs/reports/`. That observation window and write-up require a
   reachable staging environment and real elapsed time, neither of
   which is available in this change — this PR ships the config
   overlay, the build/deploy procedure, and the bootstrap steps needed
   to *start* that observation, but the observation itself and the
   resulting report are still open. Whoever runs staging should follow
   steps 1–4 above, let it soak, and produce the lift write-up as a
   separate follow-up.

## References

- [ADR-012](../ADRs/ADR-012-tiered-categorization-engine.md) — Tier 2 architecture
- [ADR-017](../ADRs/ADR-017-sona-enhanced-flow-detection.md) — Flow pattern matcher
- [Phase 0 spike](../spikes/ruvector-phase0.md) — research notes on ruvector / sona APIs
