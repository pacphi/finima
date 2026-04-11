# ADR-001: Rust Multi-Crate Workspace Architecture

**Status:** Accepted  
**Date:** 2026-04-10  
**Deciders:** Chris Phillipson

---

## Context

Finima's backend must handle authentication, file parsing (5+ formats), LLM orchestration, financial analysis, RSS feed aggregation, and a REST/WebSocket API layer. A monolithic single-binary approach would lead to long compile times, tangled dependencies, and difficulty testing individual subsystems in isolation.

## Decision

Organize the backend as a **Cargo workspace** with 8 focused crates:

| Crate             | Responsibility                                                                           |
| ----------------- | ---------------------------------------------------------------------------------------- |
| `finima-core`     | Domain models, business logic, error types, repository traits                            |
| `finima-db`       | SQLx + PostgreSQL: migrations, connection pool, repository implementations               |
| `finima-api`      | Axum HTTP server, routes, middleware, WebSocket, extractors                              |
| `finima-auth`     | Magic link generation/validation, JWT, Resend client                                     |
| `finima-ingest`   | File parsers (OFX, QFX, QIF, CSV, XLS/XLSX), dedup, preview                              |
| `finima-llm`      | LLM client abstraction (Candle/mistral.rs + Ollama), prompts, tool-calling orchestration |
| `finima-analysis` | Recurring detection, budget computation, health scoring, flow detection                  |
| `finima-feed`     | RSS/Atom fetching, article summarization, relevance scoring                              |

**Dependency direction flows inward:** `finima-api` depends on all other crates. `finima-db` depends on `finima-core`. Feature crates (`auth`, `ingest`, `llm`, `analysis`, `feed`) depend on `finima-core` and optionally on `finima-db`.

## Consequences

**Positive:**

- Parallel compilation of independent crates reduces rebuild times during development.
- Each crate can be unit-tested in isolation with minimal dependencies.
- Clear ownership boundaries make it easier to reason about changes and their blast radius.
- `finima-core` defines traits (e.g., `TransactionRepository`) that `finima-db` implements, enabling test doubles.

**Negative:**

- Cross-crate refactoring requires updating multiple `Cargo.toml` files.
- Initial workspace setup has more boilerplate than a single crate.
- `sqlx` compile-time query checking requires `DATABASE_URL` set even when building non-DB crates (mitigated by `sqlx::offline` mode).

## Alternatives Considered

1. **Single crate with modules** — Simpler setup but long compile times, no isolation. Rejected.
2. **Microservices** — Overengineered for a self-hosted 1-10 user app. Network overhead and deployment complexity unjustified. Rejected.
3. **Workspace with fewer crates (3-4)** — Considered merging `auth` into `api` and `feed` into `analysis`, but the LLM dependency boundary and Resend client warranted separation.
