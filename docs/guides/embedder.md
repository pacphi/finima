# Embedder Configuration

Finima's Tier 2 semantic categorization and the ADR-017 flow-pattern
matcher both consume vectors from a pluggable embedder. This guide
covers selection, configuration, and operational tuning.

## Backends

| Backend  | Use case                                         | Deps                                  | Setup                                |
| -------- | ------------------------------------------------ | ------------------------------------- | ------------------------------------ |
| `none`   | CI / tests / BYO vectors                         | none                                  | always available                     |
| `ollama` | Local HTTP — reuses the existing Ollama service  | `reqwest`                             | run `ollama pull nomic-embed-text`   |
| `candle` | Local in-process BertModel                       | `candle-*`, `hf-hub`, `tokenizers`    | first run downloads the model        |

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
    backend: jaccard           # jaccard | ruvector
    dim: 384                   # must match embedder.dim for ruvector
    hnsw_m: 32
    hnsw_ef_construction: 200
    hnsw_ef_search: 100
    bootstrap_on_start: true
    bootstrap_max_examples: 0  # 0 = unbounded

embedder:
  backend: none                # none | ollama | candle
  dim: 384                     # must match categorize.tier2.dim for RuVector
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

## References

- [ADR-012](../ADRs/ADR-012-tiered-categorization-engine.md) — Tier 2 architecture
- [ADR-017](../ADRs/ADR-017-sona-enhanced-flow-detection.md) — Flow pattern matcher
- [Phase 0 spike](../spikes/ruvector-phase0.md) — research notes on ruvector / sona APIs
