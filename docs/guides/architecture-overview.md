# Architecture Overview

This document describes the high-level architecture of Finima, a personal
finance application built with Rust (backend) and React (frontend).

## System Diagram

```text
                          Internet
                             |
                        [ Caddy ]
                      (reverse proxy,
                       auto-TLS)
                       /         \
                      /           \
               /api/*             /*
               /ws/*
                |                  |
          [ Backend ]        [ Frontend ]
          (Axum, :3000)      (nginx, :80)
          /    |     \
         /     |      \
   [ PostgreSQL ]  [ MinIO ]  [ Ollama ]
     (data)       (files)    (LLM inference)
```

Traffic enters through Caddy, which terminates TLS and routes API/WebSocket
requests to the Axum backend and all other requests to the frontend served by
nginx. The backend communicates with PostgreSQL for data, MinIO for file
storage, and Ollama for LLM inference.

## Crate Dependency Graph

The backend is organized as a Cargo workspace with eight crates:

```text
                    finima-core
                   (domain types,
                    traits, errors)
                  /   |   |   \    \
                 /    |   |    \    \
  finima-db   finima- finima- finima- finima-
  (repos,     auth   ingest  llm     analysis
   migrations)(JWT,  (CSV,   (Ollama (budgets,
              magic  OFX,    client, recurring,
              links) QIF,    categ.) dashboards)
                     Excel)
                                |
                            finima-feed
                            (RSS polling,
                             summarization)
                                |
                         finima-api
                         (Axum server,
                          handlers,
                          router -- depends
                          on ALL crates)
```

**Key relationships:**

- `finima-core` has no internal dependencies. It defines domain models
  (`User`, `Portfolio`, `Account`, `Transaction`, etc.), repository traits,
  and error types.
- `finima-db` implements the repository traits defined in `finima-core`.
- `finima-feed` depends on `finima-llm` for article summarization.
- `finima-api` is the composition root. It depends on every other crate and
  wires them together via dependency injection into Axum's state.

## Request Lifecycle

A typical authenticated API request flows through these layers:

```text
Client
  |
  v
Caddy (TLS termination, routing)
  |
  v
Axum Router
  |-- Security headers (X-Content-Type-Options, X-Frame-Options, HSTS)
  |-- Body size limit (1 MB default, 50 MB for uploads)
  |-- Request tracing (tower-http TraceLayer)
  |-- CORS enforcement
  |-- Prometheus metrics middleware (counters, histograms, in-flight gauge)
  |-- JWT secret injection middleware
  |
  v
Route Handler
  |-- AuthUser extractor (validates JWT from Authorization header)
  |-- Request body deserialization (serde)
  |-- Ownership verification (helpers.rs)
  |-- Business logic (delegates to domain crates)
  |-- Repository call (finima-db, via AppState pool)
  |
  v
PostgreSQL (sqlx query execution)
  |
  v
Response serialization (JSON) -> Client
```

Unauthenticated endpoints (`/health`, `/metrics`, `/api/auth/*`) skip the
`AuthUser` extraction step.

## Data Model

### Core Entities

```text
User
  |-- has many --> Portfolio
                     |-- has many --> Account
                                       |-- has many --> Transaction
                                       |-- has many --> AccountFlow

User
  |-- has many --> Budget
  |-- has many --> SavingsGoal
  |-- has many --> UserCategoryOverride
  |-- has many --> Upload
  |-- has many --> Session
```

### Key Entity Descriptions

| Entity               | Table                     | Purpose                                            |
| -------------------- | ------------------------- | -------------------------------------------------- |
| User                 | `users`                   | Application user, identified by email              |
| Portfolio            | `portfolios`              | Groups accounts (e.g., "Personal", "Business")     |
| Account              | `accounts`                | Financial account (checking, savings, credit card) |
| Transaction          | `transactions`            | Individual financial transaction with category     |
| Upload               | `uploads`                 | File upload metadata and processing state          |
| RecurringGroup       | `recurring_groups`        | Detected recurring transaction patterns            |
| Budget               | `budgets`                 | Monthly spending target per category               |
| SavingsGoal          | `savings_goals`           | User-defined savings target                        |
| UserCategoryOverride | `user_category_overrides` | User corrections to LLM categorization             |
| AccountFlow          | `account_flows`           | Inter-account money movement                       |
| FlowGroup            | `flow_groups`             | Named group of related flows                       |
| MagicLink            | `magic_links`             | Passwordless auth token                            |
| Session              | `sessions`                | Active user session / refresh token                |

All monetary amounts are stored as `NUMERIC` in PostgreSQL and represented as
`rust_decimal::Decimal` in Rust. The `f64` type is never used for money.

## Authentication Flow

Finima uses passwordless authentication via magic links (see
[ADR-002](../ADRs/ADR-002-passwordless-auth-magic-links.md)).

```text
1. User enters email
   |
   v
2. POST /api/auth/magic-link  (rate-limited: 5/min per IP)
   |-- Generate random token
   |-- Hash token, store in magic_links table (expires in 15 min)
   |-- Send email via Resend API with link containing token
   |
   v
3. User clicks link in email
   |
   v
4. POST /api/auth/verify  (token in request body)
   |-- Look up hashed token in magic_links table
   |-- Verify not expired, not already used
   |-- Create or find User record
   |-- Create Session record
   |-- Issue JWT (access token) + refresh token
   |-- Mark magic link as used
   |
   v
5. Client stores JWT, sends in Authorization header
   |
   v
6. POST /api/auth/refresh  (when JWT expires)
   |-- Validate refresh token against sessions table
   |-- Issue new JWT
   |
   v
7. DELETE /api/auth/session  (logout)
   |-- Delete session record
```

## File Import Pipeline

Finima supports importing transactions from CSV, OFX/QFX, QIF, and Excel files
(see [ADR-005](../ADRs/ADR-005-multi-format-file-import.md)).

```text
1. POST /api/uploads  (multipart file upload, 50 MB limit)
   |-- Store file in MinIO (finima-uploads bucket)
   |-- Create upload record (status: pending)
   |-- Detect format from extension/content
   |
   v
2. Parse file (finima-ingest crate)
   |-- CSV: detect delimiter, parse with column mapping
   |-- OFX/QFX: XML parse with financial-specific schema
   |-- QIF: line-oriented parse (Quicken Interchange Format)
   |-- Excel: calamine crate for .xls/.xlsx
   |-- Produce Vec<RawTransaction> with amounts as Decimal
   |-- Compute file hash (SHA-256) for duplicate detection
   |
   v
3. GET /api/uploads/{id}/preview
   |-- Return parsed transactions for user review
   |-- User can adjust column mappings or confirm
   |
   v
4. POST /api/uploads/{id}/confirm
   |-- Insert transactions into database
   |-- Trigger categorization (async)
   |-- Update upload status
   |-- Send progress via WebSocket
   |
   v
5. Categorization (finima-llm crate)
   |-- First pass: pattern matching against user overrides
   |-- Second pass: batch uncategorized to Ollama LLM
   |-- Store categories on transaction records
   |
   v
6. GET /api/uploads/{id}/status
   |-- Poll or receive WebSocket updates on progress
```

## LLM Integration

Transaction categorization uses a two-tier approach. For the full pipeline
walkthrough — prompt structure, tool schema, confidence scoring, on-demand
triggers, and source file map — see the
[Categorization Guide](./categorization.md). Architecture decision:
[ADR-003](../ADRs/ADR-003-local-llm-gemma4-categorization.md).

### Tier 1: Pattern Matching

Before calling the LLM, the system checks `user_category_overrides` for
exact or substring matches on the transaction description. User corrections
take priority over LLM suggestions.

### Tier 2: LLM Batch Categorization

Uncategorized transactions are sent to Ollama in batches. The LLM (Gemma 4
by default) receives transaction descriptions and returns category
assignments. Results are stored on the transaction records.

The LLM client (`finima-llm`) communicates with Ollama via its HTTP API.
If Ollama is unavailable, no model is loaded, or neither the `ollama` nor
`candle` compile-time feature flag is enabled, a **stub client** is used
instead. The stub returns `category="other"` with `confidence=0.5` for all
categorization requests and placeholder values for enrichment and
summarization. This is logged at startup (`Using STUB LLM client`).
Transactions imported during stub mode can be re-categorized later using the
on-demand categorization trigger (`POST /api/transactions/categorize`) once a
real LLM backend is available.

### Article Summarization

The `finima-feed` crate reuses the LLM client for summarizing RSS feed
articles. It fetches articles on a configurable polling interval (default: 6
hours) and generates concise summaries.

## Real-Time Updates

WebSocket connections provide real-time progress updates during long-running
operations like file imports and batch categorization (see
[ADR-007](../ADRs/ADR-007-websocket-realtime-progress.md)).

```text
Client                                    Server
  |                                         |
  |--- GET /api/ws?token=<jwt> ------------>|
  |                                         |-- Validate JWT from query param
  |<---- WebSocket upgrade -----------------|
  |                                         |
  |<---- {"type":"upload_progress",...} -----|  (during file import)
  |<---- {"type":"categorize_progress",...} -|  (during categorization)
  |                                         |
```

The WebSocket endpoint authenticates via a JWT in the query string (not the
Authorization header) because the browser WebSocket API does not support custom
headers.

## Observability

### Metrics Collection

The backend exposes Prometheus-format metrics at `GET /metrics`:

- **http_requests_total**: Counter by method, path template, status code
- **http_request_duration_seconds**: Histogram with exponential buckets
- **http_requests_in_flight**: Gauge of concurrent requests
- **error_rate_5xx**: Counter for error budget tracking

Path templates (e.g., `/api/users/{id}`) are used instead of concrete paths to
keep metric cardinality bounded.

### SigNoz Integration

The observability stack (`docker-compose.observability.yml`) adds:

- **OpenTelemetry Collector**: Scrapes `/metrics` and forwards to SigNoz
- **SigNoz**: Dashboard UI on port 3301
- **ClickHouse**: Time-series storage backend

### Dashboard Personas

| Persona           | Key Metrics                                 |
| ----------------- | ------------------------------------------- |
| On-call engineer  | Error rate, p99 latency, in-flight requests |
| Product owner     | Request volume, endpoint usage patterns     |
| Capacity planning | Connection pool usage, Ollama queue depth   |

## Key Design Decisions

Significant architectural choices are documented as ADRs:

| ADR                                                              | Decision                                                        |
| ---------------------------------------------------------------- | --------------------------------------------------------------- |
| [ADR-001](../ADRs/ADR-001-rust-multi-crate-workspace.md)         | Multi-crate workspace for modularity and compile-time isolation |
| [ADR-002](../ADRs/ADR-002-passwordless-auth-magic-links.md)      | Magic links instead of passwords for simpler, more secure auth  |
| [ADR-003](../ADRs/ADR-003-local-llm-gemma4-categorization.md)    | Local Ollama LLM for privacy-preserving categorization          |
| [ADR-004](../ADRs/ADR-004-postgresql-single-datastore.md)        | PostgreSQL as the sole data store (no Redis, no secondary DB)   |
| [ADR-005](../ADRs/ADR-005-multi-format-file-import.md)           | Support CSV, OFX, QIF, and Excel with format auto-detection     |
| [ADR-006](../ADRs/ADR-006-react-vite-zustand-frontend.md)        | React + Vite + Zustand for fast, lightweight frontend           |
| [ADR-007](../ADRs/ADR-007-websocket-realtime-progress.md)        | WebSocket for import/categorization progress                    |
| [ADR-008](../ADRs/ADR-008-inter-account-flow-detection.md)       | Inter-account flow detection and Sankey visualization           |
| [ADR-009](../ADRs/ADR-009-externalized-yaml-configuration.md)    | YAML config files with env var override layering                |
| [ADR-010](../ADRs/ADR-010-candle-mistralrs-inference-backend.md) | In-process Candle/mistral.rs inference as Ollama alternative    |

Domain boundaries and bounded contexts are documented in the
[DDD index](../DDDs/README.md).
