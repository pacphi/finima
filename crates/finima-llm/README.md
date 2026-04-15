# finima-llm

Ollama LLM client for transaction categorization, recurring payment enrichment, and financial insight generation.

## Purpose

This crate provides a trait-based abstraction over LLM backends (Candle and Ollama) for AI-powered features: batch transaction categorization with tool-call parsing, recurring payment enrichment (merchant identification, subscription detection), and free-form financial insight generation. When the provider is set to `none`, no LLM is loaded and categorization relies on Tiers 0-2 (merchant lookup, pattern engine, semantic search).

## Key Types / Modules

| Module           | Description                                                                                                                                                                                                                                                                                             |
| ---------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `client.rs`      | `LlmClient` async trait with `categorize_batch`, `enrich_recurring`, `generate_insight`; `OllamaClient` implementation; core DTOs: `TransactionInput`, `CategorizationBatch`, `CategorizationResult`, `OverridePattern`, `RecurringGroupCandidate`, `RecurringEnrichment`                               |
| `categorizer.rs` | `Categorizer` -- orchestrates the full categorization pipeline: pattern-matching overrides first, then LLM for remaining transactions in configurable batch chunks (default 20); produces `CategorizationReport` with counts for pattern-matched, LLM-categorized, flagged (low confidence), and failed |
| `enricher.rs`    | `normalize_merchant()` -- rule-based merchant name normalization (strips prefixes like `SQ *`, `AMZN*`, `TST*`; expands abbreviations like `WHOLEFDS`; applies title case)                                                                                                                              |
| `prompts.rs`     | Prompt builders: `build_categorization_system_prompt`, `build_categorization_user_prompt`, `build_enrichment_prompt`, `build_insight_prompt`                                                                                                                                                            |
| `tool_defs.rs`   | JSON tool definitions for the `categorize_transaction` and `enrich_recurring` functions sent to the LLM; defines all 18 spending categories                                                                                                                                                             |
| `error.rs`       | `LlmError` enum for timeout, HTTP, parsing, and tool-call errors                                                                                                                                                                                                                                        |

## Dependencies

Depends on **finima-core** for `AppError` conversion. Uses `reqwest` for HTTP calls to the Ollama API, `serde`/`serde_json` for request/response serialization, and `async-trait` for the `LlmClient` trait.

## Developer Top-of-Mind

- **Tool calls are mapped by `transaction_index`**: the LLM returns tool calls referencing transactions by their index in the batch. Off-by-one errors here cause miscategorization.
- **Retry with exponential backoff** on timeout and 5xx responses from Ollama. Do not retry on 4xx or parse errors.
- **No stub fallback.** When `provider = "none"`, no LLM client is created. Tiers 0-2 handle categorization; unmatched transactions remain uncategorized (category = NULL) until a real LLM is configured or the user manually categorizes them.
- **Override patterns take priority over LLM**: the `Categorizer` checks user-defined patterns (case-insensitive substring match) before sending remaining transactions to the LLM.
- **Batch size** defaults to 20 transactions per LLM call and is configurable via `with_batch_size()`. Larger batches are chunked automatically.
- **Low-confidence results** (below 0.7) are flagged in the `CategorizationReport` for user review.

## Testing

```sh
cargo test -p finima-llm
```

Tests use mock `LlmClient` implementations (in-crate) to verify pattern matching, batch chunking, override priority, confidence flagging, merchant normalization, prompt construction, and tool definition validity. No running LLM service required.

## Categorization Pipeline

1. The `Categorizer` receives a list of `TransactionInput` and `OverridePattern` entries
2. Each transaction is checked against override patterns (case-insensitive substring match)
3. Matched transactions get confidence 1.0 and skip the LLM entirely
4. Remaining transactions are chunked into batches (default size 20)
5. Each batch is sent to the LLM with tool definitions for `categorize_transaction`
6. The LLM returns tool calls mapped by `transaction_index`
7. Results with confidence below 0.7 are flagged for user review
8. A `CategorizationReport` summarizes counts: pattern-matched, LLM-categorized, flagged, failed

## Merchant Normalization Rules

| Pattern       | Normalized To         |
| ------------- | --------------------- |
| `SQ *<name>`  | `<Name>` (title case) |
| `AMZN*...`    | `Amazon`              |
| `TST*<name>`  | `<Name>` (title case) |
| `WHOLEFDS...` | `Whole Foods Market`  |
| Other         | Title case applied    |
