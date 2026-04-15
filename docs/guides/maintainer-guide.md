# Maintainer Guide

This guide covers the day-to-day workflow for developers contributing to Finima.

## Development Setup

### Prerequisites

| Tool             | Version        | Purpose                        |
| ---------------- | -------------- | ------------------------------ |
| Rust             | stable (1.82+) | Backend compilation            |
| Node.js          | 24+            | Frontend tooling               |
| pnpm             | 10+            | Frontend package manager       |
| Docker & Compose | latest         | PostgreSQL, Ollama, MinIO      |
| sqlx-cli         | latest         | Database migrations            |
| Ollama           | latest         | Local LLM inference (optional) |

Install sqlx-cli if you do not have it:

```sh
cargo install sqlx-cli --no-default-features --features postgres
```

### First-Time Setup

```sh
# 1. Clone the repository
git clone https://github.com/pacphi/finima.git && cd finima

# 2. Create an environment file (optional -- defaults work out of the box)
cp .env.example .env   # review and edit if you need custom passwords or a Resend API key

# 3. Install backend and frontend dependencies
make install

# 4. Run database migrations (requires PostgreSQL -- next step starts it)
# make migrate

# 5. Start everything (infrastructure + backend + frontend)
make start
```

`make start` brings up PostgreSQL and MinIO (and Ollama when
`LLM=ollama`), waits for them to be healthy, then launches the backend
(port 3000) and frontend (port 5173) together. Press `Ctrl-C` to stop.

If you prefer to manage infrastructure separately:

```sh
make docker-infra   # Start PostgreSQL + MinIO (and Ollama when LLM=ollama)
make migrate        # Run database migrations
make dev            # Start backend + frontend
```

To run only the backend (useful when working on backend code):

```sh
make dev-backend
```

The backend loads configuration from individual YAML files in `config/`
(`server.yaml`, `database.yaml`, `auth.yaml`, `llm.yaml`, `storage.yaml`,
`categories.yaml`, `services.yaml`, `logging.yaml`), then applies environment
overlays (`development.yaml`, `production.yaml`), and finally environment
variables prefixed with `APP__` (e.g. `APP__DATABASE__URL` overrides
`database.url`). No `.env` file is required for local development if you keep
the defaults.

If you are not using Make, you can start the app directly:

```sh
docker compose up -d                       # Start PostgreSQL, Ollama, MinIO
APP_ENV=development cargo run --bin finima-api  # Start backend
cd frontend && pnpm dev                    # Start frontend (separate terminal)
```

### LLM Backend Configuration

Finima can optionally use an LLM to categorize transactions, enrich recurring
payment metadata, and generate spending insights. **By default, no LLM is
configured** -- the application runs with Tiers 0-2 (merchant lookup, pattern
engine, and semantic search) which handle 80-95% of transactions. Uncategorized
transactions can be manually categorized via the UI.

The default configuration in `config/llm.yaml` is `provider: "none"`. Three LLM
backends are available, selected by the `llm.provider` field in `config/llm.yaml`
(or the `APP__LLM__PROVIDER` environment variable):

- `make start` -- runs without LLM (default, `LLM=none`)
- `make start LLM=ollama` -- enables Ollama for AI categorization
- `make start LLM=candle` -- enables Candle for AI categorization

#### Option 1: Candle (in-process inference)

The Candle backend uses [mistral.rs](https://github.com/EricLBuehler/mistral.rs)
to run model inference directly inside the Finima process, removing the HTTP
round-trip to Ollama. This is the default because it requires no sidecar
container and the Makefile auto-detects your GPU.

```sh
make dev                 # auto-detects Metal (macOS) / CUDA (NVIDIA) / CPU
make dev-candle          # explicit alias, same behavior
make dev LLM=candle-metal   # force Metal
make dev LLM=candle-cuda    # force CUDA
make dev LLM=candle         # force CPU-only
```

When using Candle, `make docker-infra` starts only PostgreSQL and MinIO (the
Ollama container is skipped).

On first startup the model downloads from HuggingFace Hub (~4-5 GB for the
default Gemma 4 E4B at Q4_K_M quantization). To use a local GGUF file instead,
set `llm.candle.model_path` to the file path.

If you run `cargo` directly (without Make), pass the feature flag explicitly:

```sh
cargo run -p finima-api --features candle,metal
```

Configuration keys (YAML path / env var):

| YAML key                    | Env var                            | Default                 |
| --------------------------- | ---------------------------------- | ----------------------- |
| `llm.candle.model_id`       | `APP__LLM__CANDLE__MODEL_ID`       | `google/gemma-4-E4B-it` |
| `llm.candle.model_path`     | `APP__LLM__CANDLE__MODEL_PATH`     | (empty = use HF Hub)    |
| `llm.candle.quantization`   | `APP__LLM__CANDLE__QUANTIZATION`   | `Q4_K_M`                |
| `llm.candle.device`         | `APP__LLM__CANDLE__DEVICE`         | `auto`                  |
| `llm.candle.context_length` | `APP__LLM__CANDLE__CONTEXT_LENGTH` | `8192`                  |
| `llm.candle.threads`        | `APP__LLM__CANDLE__THREADS`        | `0` (auto-detect)       |

Hardware requirements:

- **Apple Silicon (M1/M2/M3/M4):** Use `--features metal`. 16 GB unified memory
  is recommended.
- **NVIDIA GPU:** Use `--features cuda`. Requires CUDA toolkit installed. 8 GB+
  VRAM recommended.
- **CPU only:** Works but is significantly slower. Adequate for light workloads
  or testing.

#### Option 2: Ollama (HTTP inference)

Ollama runs as a Docker container alongside PostgreSQL and MinIO.

```sh
make dev LLM=ollama      # starts Ollama container, compiles with --features ollama
make dev-ollama           # explicit alias
```

Then pull a model into the container:

```sh
make download-model LLM=ollama          # pulls gemma4:26b-a4b-it-q4_K_M
# or manually:
docker exec finima-ollama ollama pull gemma4:26b-a4b-it-q4_K_M
```

If you run `cargo` directly, pass the feature flag explicitly:

```sh
cargo run -p finima-api --features ollama
```

Configuration keys (YAML path / env var):

| YAML key           | Env var                   | Default                    |
| ------------------ | ------------------------- | -------------------------- |
| `llm.ollama.url`   | `APP__LLM__OLLAMA__URL`   | `http://localhost:11434`   |
| `llm.ollama.model` | `APP__LLM__OLLAMA__MODEL` | `gemma4:26b-a4b-it-q4_K_M` |

#### Option 3: No LLM

```sh
make dev LLM=none         # compiles without any LLM feature
make dev-no-llm           # explicit alias
```

If neither the `candle` nor `ollama` feature is enabled at compile time, or if
the provider is set to `"none"`, no LLM is loaded. In this mode:

- Tiers 0-2 (merchant lookup, pattern engine, semantic search) still categorize
  transactions normally, handling 80-95% of common transactions.
- Transactions that do not match any tier remain uncategorized (category = NULL).
- Recurring payment enrichment and insight generation are unavailable.

The application is fully functional otherwise -- you can import statements, view
transactions, set budgets, and manually categorize entries. Uncategorized
transactions can be re-processed once a real LLM is configured, either by
re-importing the file or using the on-demand categorization endpoint
(`POST /api/transactions/categorize`).

#### Feature flag summary

The LLM backend is controlled by Cargo feature flags on both `finima-llm` and
`finima-api`. The features cascade:

| Feature flag       | Activates          | Extra native deps                   |
| ------------------ | ------------------ | ----------------------------------- |
| `ollama` (default) | Ollama HTTP client | None (uses `reqwest`)               |
| `candle`           | mistral.rs backend | `mistralrs`, `hf-hub`, `tokenizers` |
| `metal`            | Candle + Metal GPU | Metal framework (macOS)             |
| `cuda`             | Candle + CUDA GPU  | CUDA toolkit (Linux)                |

The `finima-api` crate re-exports these as pass-through features. When you run
`cargo run -p finima-api --features candle,metal`, the flag propagates to
`finima-llm` automatically.

#### Choosing a backend

| Criterion        | Ollama                      | Candle                       |
| ---------------- | --------------------------- | ---------------------------- |
| Setup effort     | Low (Docker container)      | Medium (native compile)      |
| Latency          | HTTP round-trip per request | In-process, lower latency    |
| GPU support      | Managed by Ollama           | `--features metal` or `cuda` |
| Model management | `make models LLM=ollama`    | `make models LLM=candle`     |
| Production use   | Needs sidecar container     | Single binary, no sidecar    |

### Merchant Audit Tool

The `merchant-audit` CLI binary helps maintainers identify uncategorized
merchants and find candidates for adding to the seed data. It connects to the
database, loads the current seed merchant registry, and prints a report
covering:

- Total, categorized, and uncategorized transaction counts
- Tier distribution (merchant_lookup, pattern_engine, llm, etc.)
- Top uncategorized descriptions with occurrence counts
- Suggested new seed merchants -- LLM-categorized merchants not yet in
  `seed_merchants.json`, printed as JSON snippets ready to paste

**How to run:**

```sh
cargo run --bin merchant-audit
```

The tool uses the same configuration loading as the main API server (YAML files
in `config/`, environment variables with `APP__` prefix, `.env` file). It is
non-interactive and prints the report to stdout.

**Example output:**

```text
Merchant Audit Report
=====================

Transactions: 816 total, 519 categorized (64%), 297 uncategorized

Tier Distribution:
  merchant_lookup       267 (51%)
  pattern_engine        116 (22%)
  llm                   136 (26%)

Top Uncategorized Descriptions:
    68x  External Withdrawal - CHASE CREDIT CRD  - EPAY
    33x  External Withdrawal - AMEX EPAYMENT ER AM - ACH PMT
    27x  External Withdrawal - BK OF AMER VISA  - ONLINE PMT
    ...

Suggested Seed Merchants (from LLM results, not in current seed data):
  {"name": "Optum", "aliases": ["OPTUM"], "category": "healthcare", "subcategory": "health_insurance"},
  {"name": "Cornerstone Bank", "aliases": ["CORNERSTONE BANK"], "category": "transfer", "subcategory": "ach_transfer"},
  ...

To add these, append them to:
  crates/finima-categorize/data/seed_merchants.json
```

**Using the output to improve seed data:**

1. Run the audit after a batch of LLM categorizations.
2. Review the suggested merchants for accuracy.
3. Copy the JSON lines into `crates/finima-categorize/data/seed_merchants.json`
   (inside the top-level array).
4. Rebuild and restart -- those merchants will now be categorized instantly by
   Tier 0 on subsequent imports, avoiding LLM calls.

## Project Structure

### Workspace Layout

```text
finima/
  Cargo.toml                  # Workspace root
  Makefile                    # Build, test, deploy commands
  config/
    server.yaml               # Server host/port
    database.yaml             # Database connection
    auth.yaml                 # Authentication settings
    llm.yaml                  # LLM provider config
    storage.yaml              # S3/MinIO settings
    categories.yaml           # Category hierarchy
    services.yaml             # Resend, feed, CORS
    logging.yaml              # Logging configuration
    development.yaml          # Dev overrides
    production.yaml           # Prod overrides
  crates/
    finima-core/              # Domain types, traits, error types
    finima-db/                # Repository implementations, migrations
    finima-auth/              # JWT, magic link, middleware
    finima-ingest/            # CSV/OFX/Excel parsing
    finima-llm/               # Ollama client, categorization logic
    finima-analysis/          # Budgets, recurring detection, dashboards
    finima-feed/              # RSS feed polling, article summarization
    finima-api/               # Axum server, handlers, router, config
  frontend/
    src/
      api/                    # API client functions per resource
      components/             # React components
      hooks/                  # Custom hooks (useApi, useFocusTrap, ...)
      routes/                 # Page-level route components
      stores/                 # Zustand stores (auth, portfolio, theme, ...)
      types/                  # TypeScript type definitions
      utils/                  # Formatting helpers (format.ts)
  docs/
    ADRs/                     # Architecture Decision Records
    DDDs/                     # Domain-Driven Design documents
    guides/                   # This file lives here
```

### Crate Dependency Graph

```text
finima-core  (domain types, traits -- no internal deps)
    |
    +-- finima-db         (repos, migrations)
    +-- finima-auth       (JWT, magic links)
    +-- finima-ingest     (file parsers)
    +-- finima-llm        (Ollama client)
    +-- finima-analysis   (budgets, recurring, dashboards)
    +-- finima-feed       (RSS polling; also depends on finima-llm)
    |
    +-- finima-api        (depends on ALL crates above)
```

`finima-core` is the foundation. Every other crate depends on it for shared
domain types and trait definitions. `finima-api` is the composition root that
wires everything together.

## Adding a New Feature

### Adding a New API Handler

1. **Define domain types** in `finima-core/src/` (models, error variants).
2. **Add repository trait** in `finima-core/src/` and implement it in
   `finima-db/src/`.
3. **Write the handler** in `finima-api/src/handlers/your_feature.rs`.
   - Re-export the module in `handlers/mod.rs`.
   - Use helpers from `handlers/helpers.rs` for ownership checks and common
     patterns.
4. **Register routes** in `router.rs` -- create a route group and nest it under
   `/api/your-feature`.
5. **Write tests** -- unit tests alongside the code, integration tests in
   `tests/`.

### Adding a Database Migration

```sh
make migrate-create name=add_widgets
```

This creates a reversible migration pair in `crates/finima-db/src/migrations/`.
Write your `up` SQL in the generated file. Migrations run automatically on
application startup in all environments.

### Adding a Frontend Route

1. Create a page component in `frontend/src/routes/`.
2. Add the route to the router configuration in `App.tsx`.
3. Add API client functions in `frontend/src/api/` if you need new endpoints.
4. Add types in `frontend/src/types/models.ts`.

## Code Conventions

### Backend (Rust)

- **Money values**: Always use `rust_decimal::Decimal`. Never use `f64` for
  financial amounts. The database column type is `NUMERIC` and sqlx maps it to
  `Decimal` automatically.
- **Ownership verification**: Every handler that accesses a user-owned resource
  must verify ownership. Use the helper functions in `handlers/helpers.rs`.
- **Handler helpers**: Shared logic (pagination, ownership checks, error
  mapping) goes in `handlers/helpers.rs`, not duplicated across handlers.
- **Error types**: Define errors in `finima-core` using `thiserror`. Handlers
  convert them into appropriate HTTP status codes.
- **Logging**: Use `tracing` macros (`tracing::info!`, `tracing::error!`, etc.)
  with structured fields. See [Logging configuration](#logging-configuration)
  below for level and format settings.

### Frontend (TypeScript/React)

- **No `any` types**: The codebase enforces strict typing. Use proper interfaces
  from `src/types/models.ts`.
- **Formatting**: All currency, date, and number formatting goes through
  `src/utils/format.ts`. Do not use raw `toLocaleString` or similar.
- **Modals**: Every modal must use the `useFocusTrap` hook for accessibility.
- **State management**: Use Zustand stores in `src/stores/`. Do not lift state
  into components when it needs to be shared.
- **API calls**: Use the typed client functions in `src/api/` and the `useApi`
  hook for loading/error states.

### General

- Tests are mandatory for new logic in both backend and frontend.
- Keep handler functions focused -- delegate business logic to the appropriate
  crate.
- Follow existing naming conventions: snake_case in Rust, camelCase in
  TypeScript.

## Testing

### Quick Reference

| Command                 | What it runs                               | Infra needed  |
| ----------------------- | ------------------------------------------ | ------------- |
| `make test`             | Backend unit + frontend unit tests         | None          |
| `make test-all`         | **Everything** (auto-starts/stops test DB) | Docker        |
| `make test-unit`        | Backend unit tests only                    | None          |
| `make test-integration` | Backend integration tests                  | Docker (auto) |
| `make test-llm`         | LLM integration tests via Ollama           | Docker (auto) |
| `make test-frontend`    | Vitest unit tests                          | None          |
| `make test-e2e`         | Playwright E2E tests                       | Full stack    |
| `make coverage`         | HTML coverage report                       | Docker        |

### Running All Tests (Recommended)

```sh
make test-all
```

This single command:

1. Starts the test PostgreSQL container (port 5433)
2. Runs all backend tests (unit + integration) against the test database
3. Runs frontend unit tests (Vitest, excluding Playwright E2E)
4. Stops the test database

No manual `docker-test-up` required.

### Backend Unit Tests

```sh
make test-unit
```

Runs `cargo test --workspace --lib`. No database or external services needed.

### Backend Integration Tests

```sh
make test-integration
```

Automatically starts the test PostgreSQL container if it is not already running,
then runs all integration tests (`--test '*'`). You do not need to manually
manage the database lifecycle.

### LLM Integration Tests

```sh
make test-llm
```

Automatically:

1. Starts an Ollama container on port 11435 (separate from dev on 11434)
2. Pulls the test model (`gemma4:e4b-it-q4_K_M` by default -- supports tool calling)
3. Runs the Ollama integration tests (`-p finima-llm --features ollama -- --ignored`)

The model is cached in a Docker volume, so subsequent runs skip the download.

Override the model or port:

```sh
make test-llm OLLAMA_TEST_MODEL=gemma4:26b-a4b-it-q4_K_M
make test-llm OLLAMA_TEST_PORT=11434    # use your dev Ollama instead
```

Environment variables `OLLAMA_URL` and `OLLAMA_TEST_MODEL` are also respected
directly by the test binary if you prefer running `cargo test` manually.

### Frontend Tests

```sh
make test-frontend       # Vitest unit tests (excludes E2E)
```

Playwright E2E specs in `frontend/e2e/` are excluded from Vitest via
`vite.config.ts`. Run them separately:

```sh
make test-e2e            # requires full stack running
```

### Coverage

```sh
make coverage            # generates HTML report at target/llvm-cov/html/
```

## CI/CD

### What CI Checks

The GitHub Actions pipeline (`.github/workflows/ci.yml`) runs on every push to
`main` and every pull request. It includes:

| Job                 | What it does                                                   | Gated? |
| ------------------- | -------------------------------------------------------------- | ------ |
| `validate-markdown` | markdownlint on all `.md` files                                | No     |
| `validate-yaml`     | yamllint on all `.yaml`/`.yml` files                           | No     |
| `backend-lint`      | `cargo fmt --check` + `cargo clippy -D warnings`               | No     |
| `backend-test`      | All backend tests (unit + integration) with PostgreSQL service | No     |
| `backend-audit`     | `cargo audit` security check                                   | No     |
| `llm-test`          | LLM integration tests with Ollama service container            | Yes    |
| `frontend-lint`     | ESLint + TypeScript type check + Prettier format check         | No     |
| `frontend-test`     | Vitest unit tests                                              | No     |
| `e2e-test`          | Playwright E2E (full stack)                                    | Yes    |

### Feature Flags for Expensive CI Jobs

Some CI jobs are slow or resource-intensive. They only run when their repository
variable is set to `"true"` in **GitHub > Settings > Variables > Actions**:

| Variable            | Enables                | Why gated                                      |
| ------------------- | ---------------------- | ---------------------------------------------- |
| `E2E_ENABLED`       | Playwright E2E tests   | Requires browser install, full stack spin-up   |
| `LLM_TESTS_ENABLED` | Ollama LLM integration | Requires model pull (~4 GB), inference is slow |

This keeps PR feedback fast (lint + unit + integration in ~2 minutes) while
allowing full validation when needed.

### Releases

Docker images are published to `ghcr.io/pacphi/finima-backend` and
`ghcr.io/pacphi/finima-frontend`. Build images locally with:

```sh
make docker-build
```

## Configuration

### Config Layering

The application uses the `config` crate with this precedence (highest wins):

1. Individual section files (`config/server.yaml`, `config/database.yaml`,
   `config/auth.yaml`, `config/llm.yaml`, `config/storage.yaml`,
   `config/categories.yaml`, `config/services.yaml`, `config/logging.yaml`)
2. `config/{APP_ENV}.yaml` -- environment-specific overrides (e.g.,
   `production.yaml`)
3. Environment variables -- prefixed with `APP__`, double underscores for
   nesting (e.g., `APP__DATABASE__PASSWORD`)

### Adding a New Config Field

1. Add the field to the appropriate section file in `config/` (e.g.,
   `config/llm.yaml` for LLM settings) with a sensible default.
2. Add the corresponding field to the `AppConfig` struct in
   `crates/finima-api/src/config.rs`.
3. If the field needs a production override, add it to `config/production.yaml`
   or document the env var.
4. Update this guide and any deployment docs.

### Key Environment Variables

| Variable                     | Purpose                                                |
| ---------------------------- | ------------------------------------------------------ |
| `APP_ENV`                    | Environment name (`development`, `test`, `production`) |
| `APP__DATABASE__HOST`        | Database hostname (default: `localhost`)               |
| `APP__DATABASE__USER`        | Database user                                          |
| `APP__DATABASE__PASSWORD`    | Database password                                      |
| `APP__DATABASE__NAME`        | Database name                                          |
| `APP__AUTH__JWT_SECRET`      | JWT signing secret                                     |
| `APP__RESEND__API_KEY`       | Resend email API key                                   |
| `APP__AUTH__FROM_EMAIL`      | Sender address for magic-link emails                   |
| `APP__AUTH__PUBLIC_URL`      | Frontend origin for magic-link URLs                    |
| `APP__LLM__OLLAMA__URL`      | Ollama server URL                                      |
| `APP__S3__ENDPOINT_URL`      | MinIO/S3 endpoint                                      |
| `APP__S3__ACCESS_KEY_ID`     | S3 access key                                          |
| `APP__S3__SECRET_ACCESS_KEY` | S3 secret key                                          |
| `APP__CORS__ALLOWED_ORIGINS` | Comma-separated allowed origins                        |
| `RUST_LOG`                   | Overrides `logging.level` from YAML (see below)        |

### Logging Configuration

The backend uses [`tracing`](https://docs.rs/tracing) with an
[`EnvFilter`](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html)
for level control. Two YAML fields govern logging:

| Field            | Values                     | Purpose                      |
| ---------------- | -------------------------- | ---------------------------- |
| `logging.level`  | EnvFilter directive string | Controls which logs appear   |
| `logging.format` | `"pretty"` or `"json"`     | Human-readable or structured |

**Level strategy per environment:**

| Config             | `level`                             | `format`          | Rationale                                   |
| ------------------ | ----------------------------------- | ----------------- | ------------------------------------------- |
| `logging.yaml`     | `warn`                              | `json`            | Production-safe baseline; quiet, structured |
| `development.yaml` | `debug,h2=warn,hyper_util=warn,...` | `pretty`          | Verbose for app code, noisy deps suppressed |
| `test.yaml`        | `warn`                              | `pretty`          | Quiet tests                                 |
| `production.yaml`  | `info`                              | _(inherits json)_ | Operational visibility without noise        |

The `level` field uses
[EnvFilter directive syntax](https://docs.rs/tracing-subscriber/latest/tracing_subscriber/filter/struct.EnvFilter.html#directives):

```text
<global_level>,<crate>=<level>,<crate>=<level>,...
```

For example, `debug,h2=warn,sqlx=warn` means "emit debug for
everything, except `h2` and `sqlx` which only emit warnings."
Per-crate directives override the global default.

**Runtime override:** Set the `RUST_LOG` environment variable to
bypass the YAML config entirely:

```bash
RUST_LOG=debug,tower_http=info cargo run --bin finima-api
```

**Common noisy crates to suppress in development:**

| Crate        | What it logs at debug                             |
| ------------ | ------------------------------------------------- |
| `h2`         | HTTP/2 frame-level protocol events                |
| `hyper`      | HTTP connection lifecycle                         |
| `hyper_util` | Connection pooling internals                      |
| `reqwest`    | HTTP client request details                       |
| `rustls`     | TLS handshake negotiation                         |
| `tower_http` | Per-request start/finish events from `TraceLayer` |
| `sqlx`       | Every SQL query with bind parameters              |

## Common Makefile Targets

| Target                       | Description                                                             |
| ---------------------------- | ----------------------------------------------------------------------- |
| `make help`                  | Show all available targets                                              |
| `make install`               | Install backend + frontend dependencies                                 |
| `make start`                 | Start everything (infra + backend + frontend)                           |
| `make dev`                   | Start backend + frontend (assumes infra running)                        |
| `make dev-backend`           | Start backend API server only                                           |
| `make dev-watch`             | Start backend with auto-reload (needs cargo-watch)                      |
| `make build`                 | Build backend (debug) + frontend                                        |
| `make test`                  | Run unit tests (backend + frontend)                                     |
| `make test-all`              | Run ALL tests (auto-manages test DB)                                    |
| `make test-integration`      | Run integration tests (auto-starts DB if needed)                        |
| `make test-llm`              | Run LLM tests (auto-starts Ollama, pulls model)                         |
| `make lint`                  | Lint everything (Rust + TypeScript + Markdown + YAML)                   |
| `make format`                | Format all code and docs                                                |
| `make ci`                    | Full CI pipeline locally                                                |
| `make docker-infra`          | Start dev infrastructure (PostgreSQL + MinIO; Ollama when `LLM=ollama`) |
| `make docker-infra-down`     | Stop dev infrastructure                                                 |
| `make models`                | List downloaded models (set `LLM=candle` or `ollama`)                   |
| `make download-model`        | Download the default model (set `LLM=candle` or `ollama`)               |
| `make docker-up`             | Start full production stack                                             |
| `make docker-down`           | Stop production stack                                                   |
| `make migrate`               | Run database migrations                                                 |
| `make migrate-create name=x` | Create a new migration                                                  |
| `make migrate-revert`        | Revert the last migration                                               |
| `make db-seed`               | Load test seed data                                                     |
| `make coverage`              | Generate test coverage report                                           |
| `make audit`                 | Security audit all dependencies                                         |
| `make observability`         | Start SigNoz observability stack                                        |
| `make clean-all`             | Clean build artifacts + Docker volumes (destructive)                    |

## ADRs and DDDs

### Architecture Decision Records (ADRs)

ADRs capture significant technical decisions. The full index is at
`docs/ADRs/README.md`.

**When to create an ADR:**

- Choosing a new technology or library
- Changing an architectural pattern
- Making a trade-off that future maintainers need to understand

**How to add one:**

1. Copy an existing ADR as a template.
2. Number it sequentially (e.g., `ADR-010-your-topic.md`).
3. Fill in the Status, Context, Decision, and Consequences sections.
4. Add it to the index table in `docs/ADRs/README.md`.

### Domain-Driven Design Documents (DDDs)

DDDs define bounded contexts and their responsibilities. The full index and
context map are at `docs/DDDs/README.md`.

**When to create a DDD:**

- Adding a new bounded context or crate with distinct domain logic
- Significantly expanding an existing context

**How to add one:**

1. Copy an existing DDD as a template.
2. Number it sequentially (e.g., `DDD-007-your-context.md`).
3. Document the bounded context, aggregates, entities, and domain events.
4. Map its relationships to other contexts.
5. Add it to the index table in `docs/DDDs/README.md`.
