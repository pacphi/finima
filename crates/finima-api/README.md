# finima-api

Axum HTTP server with REST handlers, WebSocket support, metrics, rate limiting, and S3 storage integration.

## Purpose

This is the application's HTTP entry point. It wires together all other crates into a running Axum server, defines the full route tree under `/api/`, and provides cross-cutting concerns like CORS, security headers, Prometheus metrics, JWT injection, and rate limiting. Every user-facing endpoint lives here.

## Key Types / Modules

| Module                     | Description                                                                                                                                          |
| -------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------- |
| `main.rs`                  | Server bootstrap -- loads config, creates pool, builds router, binds listener                                                                        |
| `router.rs`                | `build_router()` -- assembles all route groups, attaches middleware layers (CORS, HSTS, body limits, tracing, metrics, JWT injection, rate limiting) |
| `state.rs`                 | `AppState` -- shared state holding PgPool, all Pg\*Repos, config, email sender, LLM client, WebSocket manager, and S3 object storage                 |
| `config.rs`                | `AppConfig` -- typed configuration loaded from YAML/env vars                                                                                         |
| `metrics.rs`               | `MetricsRegistry` -- Prometheus counters, histograms, and gauges for HTTP requests, duration, in-flight, and 5xx errors                              |
| `storage.rs`               | `ObjectStorage` -- S3-compatible file storage for upload blobs                                                                                       |
| `ws.rs`                    | `WsConnectionManager` and `ws_handler` -- WebSocket upgrade and per-user connection management                                                       |
| `error_response.rs`        | Shared error response formatting                                                                                                                     |
| `handlers/helpers.rs`      | Shared handler utilities (ownership checks, pagination parsing)                                                                                      |
| `handlers/auth.rs`         | Magic link request, verification, token refresh, session deletion                                                                                    |
| `handlers/portfolios.rs`   | Portfolio CRUD                                                                                                                                       |
| `handlers/accounts.rs`     | Account CRUD with balance computation                                                                                                                |
| `handlers/transactions.rs` | Transaction listing, search, bulk update, single update                                                                                              |
| `handlers/uploads.rs`      | File upload, preview, confirm, status check (50 MB body limit)                                                                                       |
| `handlers/dashboard.rs`    | Summary, net worth, cashflow, spending aggregations                                                                                                  |
| `handlers/budgets.rs`      | Budget CRUD, budget-vs-actual, auto-suggest                                                                                                          |
| `handlers/recurring.rs`    | Recurring group listing and update                                                                                                                   |
| `handlers/savings.rs`      | Savings goal CRUD                                                                                                                                    |
| `handlers/flows.rs`        | Account flow CRUD, Sankey data, outflow ranking, balance impact; flow group CRUD                                                                     |
| `handlers/feed.rs`         | Feed listing and article summary                                                                                                                     |
| `handlers/overrides.rs`    | User category override listing and creation                                                                                                          |
| `handlers/users.rs`        | Current user profile and preference updates                                                                                                          |

## Dependencies

Depends on **all other finima crates**: `finima-core` (with `axum` feature), `finima-db`, `finima-auth`, `finima-ingest`, `finima-llm`, `finima-analysis`, and `finima-feed`. Also uses `aws-sdk-s3` for object storage and `prometheus-client` for metrics.

## Developer Top-of-Mind

- **Every handler must verify resource ownership**: before returning any resource, confirm the authenticated user owns the parent portfolio. Use helpers from `handlers/helpers.rs`.
- **`helpers.rs` has shared functions** for pagination parsing, ownership verification, and common query parameter extraction. Check here before duplicating logic.
- **Metrics middleware records all requests** using the route template (e.g., `/api/users/{id}`), not the concrete path, to keep cardinality bounded.
- **S3 storage** is used for file upload blobs. The `ObjectStorage` abstraction supports any S3-compatible backend.
- **Rate limiting** is applied to the magic-link endpoint only (5 requests/minute/IP) via an in-memory sliding window.
- **CORS in production** panics on startup if `allowed_origins` is empty, preventing accidental fail-open.
- Upload routes override the default 1 MB body limit with 50 MB.

## Testing

```sh
cargo test -p finima-api
```

Handler tests use `tower::ServiceExt` to send requests directly to the router without a running server. A test PostgreSQL database is required for integration tests.

## Route Map

| Method         | Path                   | Auth  | Description                       |
| -------------- | ---------------------- | ----- | --------------------------------- |
| GET            | `/health`              | No    | Health check with DB probe        |
| GET            | `/metrics`             | No    | Prometheus metrics exposition     |
| POST           | `/api/auth/magic-link` | No    | Request magic link (rate limited) |
| POST           | `/api/auth/verify`     | No    | Verify magic link token           |
| POST           | `/api/auth/refresh`    | No    | Refresh access token              |
| DELETE         | `/api/auth/session`    | Yes   | Revoke session                    |
| GET/POST       | `/api/portfolios`      | Yes   | List/create portfolios            |
| GET/PUT        | `/api/portfolios/{id}` | Yes   | Get/update portfolio              |
| GET/POST       | `/api/accounts`        | Yes   | List/create accounts              |
| GET/PUT/DELETE | `/api/accounts/{id}`   | Yes   | Get/update/archive account        |
| POST           | `/api/uploads`         | Yes   | Upload file (50 MB limit)         |
| GET            | `/api/transactions`    | Yes   | List transactions with filters    |
| GET            | `/api/ws`              | Query | WebSocket (auth via query param)  |
