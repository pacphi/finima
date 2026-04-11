# Finima — Test Plan

**Version:** 1.0 | **Date:** 2026-04-10

---

## 1. Testing Strategy Overview

```text
                    ┌─────────────────────┐
                    │     E2E Tests       │  ← Playwright (critical user flows)
                    │    (few, slow)      │
                    ├─────────────────────┤
                    │  Integration Tests  │  ← API tests against real DB + LLM mock
                    │  (moderate)         │
                    ├─────────────────────┤
                    │                     │
                    │    Unit Tests       │  ← Rust #[test] + Vitest (fast, many)
                    │   (many, fast)      │
                    │                     │
                    └─────────────────────┘
                       Testing Pyramid
```

---

## 2. Backend Test Organization

### 2.1 Unit Tests (per crate)

Located alongside source code in each crate (`#[cfg(test)] mod tests`).

**finima-core:**

- Model validation (AccountType enum exhaustiveness, Frequency parsing)
- Error type conversions
- Business rule functions (e.g., dedup hash computation)

**finima-auth:**

- Token generation produces 32-byte output
- SHA-256 hashing is deterministic
- JWT claims encode/decode roundtrip
- Token expiry validation (mock clock)
- Magic link validation: expired, already used, email mismatch

**finima-ingest:**

- OFX parser: valid OFX file → correct transaction count, amounts, dates
- OFX parser: malformed OFX → descriptive error
- QIF parser: standard QIF with D/T/P/M/L fields
- CSV parser: header detection, column mapping
- CSV parser: edge cases (quoted fields, empty rows, mixed delimiters)
- XLSX parser: multi-sheet selection, numeric date conversion
- Dedup hash: same (date, amount, description) → same hash
- Dedup hash: different transactions → different hashes

**finima-llm:**

- Prompt template rendering with variable substitution
- Tool definition JSON schema validation
- Response parsing: valid tool-call JSON → structured result
- Response parsing: malformed JSON → graceful error
- Batch chunking: 45 transactions → 3 batches of 15, 15, 15

**finima-analysis:**

- Recurring detection: monthly pattern (day 1, day 1, day 1) → Monthly
- Recurring detection: weekly pattern → Weekly
- Recurring detection: irregular intervals → Variable
- Recurring detection: single transaction → not recurring
- Budget vs. actual: simple arithmetic correctness
- Health score: boundary cases (zero income, no transactions)
- Net worth: sum across account types with sign handling (credit cards negative)
- Flow detection: matching outflow/inflow pairs across accounts (same amount, ±2 day window)
- Flow detection: no match when amounts differ by >1%
- Flow detection: no match when dates differ by >2 days
- Flow detection: one-sided flow when target account not imported
- Sankey data generation: correct source→target→amount aggregation per month
- Outflow ranking: sorted by descending monthly volume, correct % of income
- Waterfall computation: start + income − outflows = end balance
- Flow grouping: grouped flows collapse into single rollup with correct totals

**finima-feed:**

- RSS XML parsing (valid feed)
- Atom XML parsing (valid feed)
- Malformed feed → error

### 2.2 Integration Tests

Located in `tests/` directory at workspace root or per-crate `tests/` directories.

**Database integration (`finima-db`):**

- Requires: PostgreSQL via Docker (testcontainers-rs or docker-compose test profile)
- Tests run migrations, insert data, query, and verify results
- CRUD tests for each repository: users, portfolios, accounts, transactions, uploads, recurring_groups, budgets, savings_goals
- Transaction dedup: inserting same hash twice → conflict handled
- Pagination: correct offset/limit behavior
- Filtering: date range, category, account, amount range, text search

**API integration (`finima-api`):**

- Requires: running Axum server + PostgreSQL
- Use `axum::test` or `reqwest` against a test server
- Auth flow end-to-end:
  - POST `/auth/magic-link` → 200
  - POST `/auth/verify` with valid token → 200 + JWT
  - POST `/auth/verify` with expired token → 401
  - Authenticated request with valid JWT → 200
  - Authenticated request with expired JWT → 401
  - POST `/auth/refresh` → new access token
- Portfolio CRUD: create → list → get → update → delete
- Account CRUD: create under portfolio → list → get → update → archive
- Upload flow:
  - POST multipart with CSV → 200 + preview
  - POST confirm with mapping → 202 (async processing)
  - GET status → eventually "complete"
- Transaction listing: filters, sort, pagination, search
- Transaction update: category override → reflected in GET
- Bulk update: multiple transactions → all updated
- Dashboard endpoints: return correct aggregates for seeded data
- Budget: create → get vs. actual → rollover behavior
- Unauthorized access: all protected endpoints return 401 without JWT

**LLM integration (`finima-llm`):**

- Requires: Ollama running with a small test model (or mock server)
- Test with mock HTTP server returning known tool-call responses
- Test timeout handling
- Test retry logic
- For real model tests (optional, slow, CI-excluded by default):
  - Send 5 known transactions → verify categorization is reasonable
  - Verify response conforms to tool schema

### 2.3 Test Fixtures

```text
tests/
├── fixtures/
│   ├── sample.ofx              # Valid OFX file (20 transactions)
│   ├── sample.qfx              # Valid QFX file (15 transactions)
│   ├── sample.qif              # Valid QIF file (10 transactions)
│   ├── sample.csv              # Standard bank CSV (Date, Description, Amount)
│   ├── sample_chase.csv        # Chase-specific CSV format
│   ├── sample_bofa.csv         # Bank of America CSV format
│   ├── sample.xlsx             # Excel with single sheet
│   ├── sample_multisheet.xlsx  # Excel with multiple sheets
│   ├── malformed.ofx           # Truncated OFX
│   ├── malformed.csv           # Missing headers
│   ├── empty.csv               # Headers only, no data
│   ├── large.csv               # 50K rows for performance testing
│   ├── llm_response_valid.json # Mock Gemma 4 tool-call response
│   ├── llm_response_error.json # Mock error response
│   └── rss_feed.xml            # Sample RSS feed
```

---

## 3. Frontend Test Organization

### 3.1 Unit Tests (Vitest + React Testing Library)

**Stores (Zustand):**

- authStore: login sets tokens, logout clears, isAuthenticated derived
- themeStore: mode toggles, accent color updates CSS variable
- prefsStore: currency/dateFormat updates persist

**Utility functions:**

- Currency formatter: (1234.5, "USD") → "$1,234.50"
- Date formatter: respects user preference (MM/DD vs DD/MM)
- Dedup hash: client-side preview matching
- Amount parser: handles negative signs, parentheses, currency symbols

**Components:**

- `ColumnMapper`: renders correct dropdowns for CSV columns, calls onChange
- `TransactionRow`: renders category badge, inline edit triggers callback
- `BudgetProgressBar`: renders correct width and color (green < 80%, yellow < 100%, red > 100%)
- `RecurringCard`: displays frequency badge, confirm/dismiss actions
- `ThemeSwitcher`: toggles mode, updates CSS variables
- `FileDropzone`: accepts valid file types, rejects others

### 3.2 Component Integration Tests

- `SignInPage`: submit form → calls API → shows "check email" message
- `ColumnMappingModal`: receives preview data → user maps columns → calls confirm API
- `TransactionTable`: fetches data → renders rows → filter changes trigger re-fetch
- `DashboardGrid`: renders widgets in saved layout order

### 3.3 E2E Tests (Playwright)

**Critical user flows:**

| Test                          | Steps                                                                                                            |
| ----------------------------- | ---------------------------------------------------------------------------------------------------------------- |
| `auth-flow.spec.ts`           | Enter email → (mock Resend) → verify token → land on dashboard                                                   |
| `onboarding.spec.ts`          | Complete 3-step wizard → see empty dashboard with CTA                                                            |
| `csv-import.spec.ts`          | Upload CSV → map columns → confirm → see transactions in table                                                   |
| `ofx-import.spec.ts`          | Upload OFX → auto-parsed → confirm → see transactions                                                            |
| `categorize-override.spec.ts` | Click transaction category → change → verify update                                                              |
| `bulk-edit.spec.ts`           | Select 3 transactions → bulk edit category → verify all updated                                                  |
| `budget-create.spec.ts`       | Create budget → see progress bar on budget page                                                                  |
| `theme-toggle.spec.ts`        | Switch to dark mode → verify CSS variables change                                                                |
| `recurring-confirm.spec.ts`   | Navigate to recurring → confirm a pending item → moves to confirmed                                              |
| `money-flow.spec.ts`          | Tag primary account → navigate to Money Flow → see Sankey with flows → click outflow row → drill to transactions |
| `manual-flow-link.spec.ts`    | Open transaction → "Link as Transfer" → select match → flow appears in Money Flow page                           |
| `responsive.spec.ts`          | Viewport 375px → sidebar collapses → hamburger menu works                                                        |

---

## 4. Test Commands

### One-Command Test Runs

```bash
make test          # Unit tests only (no infra needed)
make test-all      # ALL tests — auto-starts/stops test DB via Docker
make test-llm      # LLM tests — auto-starts Ollama, pulls model
```

### Backend (Makefile)

```makefile
# Run all unit tests (no external services required)
test-unit:
  cargo test --workspace --lib

# Run integration tests (auto-starts test PostgreSQL if needed)
test-integration:
  # Automatically starts docker-compose.test.yml postgres on port 5433
  TEST_DATABASE_URL="postgres://finima:test@localhost:5433/finima_test" \
    APP_ENV=test cargo test --workspace --test '*'

# Run all backend tests in one pass (unit + integration)
test-all-backend:
  TEST_DATABASE_URL="postgres://finima:test@localhost:5433/finima_test" \
    cargo test --workspace

# Run LLM integration tests (auto-starts Ollama on port 11435)
test-llm:
  OLLAMA_URL="http://localhost:11435" \
  OLLAMA_TEST_MODEL="gemma4:e4b-it-q4_K_M" \
    cargo test -p finima-llm --features ollama -- --ignored

# Run specific crate tests
test-ingest:
  cargo test -p finima-ingest

test-auth:
  cargo test -p finima-auth

test-analysis:
  cargo test -p finima-analysis

# Lint
lint:
  cargo clippy --workspace --all-targets -- -D warnings
  cargo fmt --all -- --check

# Coverage
coverage:
  cargo llvm-cov --workspace --html
```

### Frontend (Makefile)

```makefile
# Unit + component tests (Vitest, excludes Playwright E2E)
test:
  pnpm run test -- --run

# Watch mode
test-watch:
  pnpm run test

# E2E tests (requires running backend)
test-e2e:
  pnpm exec playwright test

# E2E with UI
test-e2e-ui:
  pnpm exec playwright test --ui

# Lint
lint:
  pnpm run lint
  pnpm exec tsc --noEmit
```

### Environment Variables for Tests

| Variable            | Purpose                                         | Default                                             |
| ------------------- | ----------------------------------------------- | --------------------------------------------------- |
| `TEST_DATABASE_URL` | PostgreSQL connection for integration tests     | `postgres://finima:test@localhost:5433/finima_test` |
| `OLLAMA_URL`        | Ollama endpoint for LLM integration tests       | `http://localhost:11434`                            |
| `OLLAMA_TEST_MODEL` | Model for LLM tests (must support tool calling) | `gemma4:e4b-it-q4_K_M`                              |
| `APP_ENV`           | Selects config profile                          | `development`                                       |

---

## 5. Test Environments

> **All test environments are non-production.** Mock services, test doubles, and seed data are scoped exclusively to `development` and `test` profiles (`APP_ENV=development` or `APP_ENV=test`). The production profile (`APP_ENV=production`) connects only to real services and starts with an empty database populated by real user activity.

| Environment     | Database            | LLM                              | Config Profile | Purpose                                |
| --------------- | ------------------- | -------------------------------- | -------------- | -------------------------------------- |
| **Unit**        | None (mocked)       | None (mocked)                    | N/A            | Fast feedback, CI primary              |
| **Integration** | PostgreSQL (Docker) | Mock HTTP server                 | `test`         | API contract verification              |
| **E2E**         | PostgreSQL (Docker) | Ollama (optional, mock fallback) | `test`         | User flow validation                   |
| **Performance** | PostgreSQL (Docker) | Ollama (real model)              | `test`         | Latency + throughput benchmarks        |
| **Production**  | PostgreSQL (real)   | Ollama (real model)              | `production`   | Real user data only, no mocks or seeds |

---

## 6. Performance Tests

**Backend benchmarks (criterion crate):**

- CSV parsing: 10K rows → target < 5s
- OFX parsing: 5K transactions → target < 3s
- Dedup hash computation: 10K transactions → target < 1s
- Database bulk insert: 10K transactions → target < 10s
- Dashboard aggregate query: 50K transactions → target < 500ms

**Frontend performance (Lighthouse CI):**

- First Contentful Paint: < 1.5s
- Largest Contentful Paint: < 2.5s
- Time to Interactive: < 3.0s
- Cumulative Layout Shift: < 0.1

---

## 7. Test Data Seeding

> **Important:** Seed data and mock services are used **only in `development` and `test` environments**. The production configuration (`config/production.yaml`) must never include seed data, mock endpoints, or test fixtures. The `APP_ENV=production` profile disables all test seeding paths. The CI pipeline enforces this by verifying that production Docker images contain no seed scripts or test fixtures.

For integration and E2E tests, a seed script populates:

- 1 user (test@finima.local)
- 1 portfolio ("Test Portfolio")
- 3 accounts (checking, savings, credit card)
- 500 transactions across 3 months (realistic distribution)
- 5 recurring groups (rent, Netflix, Spotify, electricity, payroll)
- 8 account flow records (matching transfer pairs between checking→savings, checking→credit card)
- 1 flow group ("Housing Costs")
- 3 budget entries
- 1 savings goal

Seed script location: `tests/seed.sql` (executed by test harness before test runs, excluded from production Docker image via `.dockerignore`).

---

## 8. CI Integration

Tests run in GitHub Actions (see `06-DEPLOYMENT.md` for workflow details):

```text
CI Pipeline:
  ├── cargo fmt --check
  ├── cargo clippy
  ├── cargo test --workspace --lib          (unit tests)
  ├── docker compose up -d postgres
  ├── cargo test --workspace --test '*'     (integration tests)
  ├── cd frontend && npm ci && npm run lint
  ├── cd frontend && npm run test           (unit tests)
  ├── cd frontend && npx playwright test    (E2E, headless)
  └── cargo llvm-cov (coverage report)
```

---

## 9. Quality Gates

| Metric                     | Threshold             | Enforcement                      |
| -------------------------- | --------------------- | -------------------------------- |
| Unit test pass rate        | 100%                  | CI blocks merge                  |
| Integration test pass rate | 100%                  | CI blocks merge                  |
| E2E test pass rate         | 95% (flaky tolerance) | CI warns, manual review          |
| Code coverage (backend)    | > 70%                 | CI reports, advisory             |
| Code coverage (frontend)   | > 60%                 | CI reports, advisory             |
| Clippy warnings            | 0                     | CI blocks merge (`-D warnings`)  |
| TypeScript errors          | 0                     | CI blocks merge (`tsc --noEmit`) |
| ESLint errors              | 0                     | CI blocks merge                  |
