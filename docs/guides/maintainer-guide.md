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

# 3. Start dev services (PostgreSQL on 5432, Ollama on 11434, MinIO on 9000)
make docker-up

# 4. Install backend and frontend dependencies
make install

# 5. Run database migrations
make migrate

# 6. Start the backend API server (port 3000)
make dev

# 7. In a second terminal, start the frontend dev server (port 5173)
make -C frontend dev
```

The backend reads `config/default.yaml` automatically when `APP_ENV` is unset or
set to `development`. No `.env` file is required for local development -- the
defaults connect to the Dockerized PostgreSQL and MinIO with dev credentials.

### Optional: Ollama Models

If you want LLM-powered categorization locally:

```sh
make download-model   # pulls gemma4:26b-a4b-it-q4_K_M
```

Without a model loaded, the app falls back to pattern-based categorization.

## Project Structure

### Workspace Layout

```text
finima/
  Cargo.toml                  # Workspace root
  Makefile                    # Build, test, deploy commands
  config/
    default.yaml              # Dev defaults
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
  with structured fields.

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

1. `config/default.yaml` -- base defaults for all environments
2. `config/{APP_ENV}.yaml` -- environment-specific overrides (e.g.,
   `production.yaml`)
3. Environment variables -- prefixed with `APP__`, double underscores for
   nesting (e.g., `APP__DATABASE__URL`)

### Adding a New Config Field

1. Add the field to `config/default.yaml` with a sensible default.
2. Add the corresponding field to the `AppConfig` struct in
   `crates/finima-api/src/config.rs`.
3. If the field needs a production override, add it to `config/production.yaml`
   or document the env var.
4. Update this guide and any deployment docs.

### Key Environment Variables

| Variable                     | Purpose                                                |
| ---------------------------- | ------------------------------------------------------ |
| `APP_ENV`                    | Environment name (`development`, `test`, `production`) |
| `APP__DATABASE__URL`         | PostgreSQL connection string                           |
| `APP__AUTH__JWT_SECRET`      | JWT signing secret                                     |
| `APP__RESEND__API_KEY`       | Resend email API key                                   |
| `APP__LLM__OLLAMA__URL`      | Ollama server URL                                      |
| `APP__S3__ENDPOINT_URL`      | MinIO/S3 endpoint                                      |
| `APP__S3__ACCESS_KEY_ID`     | S3 access key                                          |
| `APP__S3__SECRET_ACCESS_KEY` | S3 secret key                                          |
| `APP__CORS__ALLOWED_ORIGINS` | Comma-separated allowed origins                        |

## Common Makefile Targets

| Target                       | Description                                           |
| ---------------------------- | ----------------------------------------------------- |
| `make help`                  | Show all available targets                            |
| `make install`               | Install backend + frontend dependencies               |
| `make dev`                   | Start backend API server                              |
| `make dev-watch`             | Start backend with auto-reload (needs cargo-watch)    |
| `make build`                 | Build backend (debug) + frontend                      |
| `make test`                  | Run unit tests (backend + frontend)                   |
| `make test-all`              | Run ALL tests (auto-manages test DB)                  |
| `make test-integration`      | Run integration tests (auto-starts DB if needed)      |
| `make test-llm`              | Run LLM tests (auto-starts Ollama, pulls model)       |
| `make lint`                  | Lint everything (Rust + TypeScript + Markdown + YAML) |
| `make format`                | Format all code and docs                              |
| `make ci`                    | Full CI pipeline locally                              |
| `make docker-up`             | Start dev services                                    |
| `make docker-down`           | Stop dev services                                     |
| `make docker-prod`           | Start production stack                                |
| `make migrate`               | Run database migrations                               |
| `make migrate-create name=x` | Create a new migration                                |
| `make migrate-revert`        | Revert the last migration                             |
| `make db-seed`               | Load test seed data                                   |
| `make coverage`              | Generate test coverage report                         |
| `make audit`                 | Security audit all dependencies                       |
| `make observability`         | Start SigNoz observability stack                      |
| `make clean-all`             | Clean build artifacts + Docker volumes (destructive)  |

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
