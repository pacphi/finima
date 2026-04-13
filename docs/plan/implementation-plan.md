# Finima — Implementation Plan

**Version:** 1.0  
**Date:** 2026-04-10  
**Total Duration:** 16 weeks (8 two-week sprints)  
**Phases:** 5 (Foundation, Intelligence, Visualization, Polish, Deployment)

---

## How to Read This Plan

Each sprint defines:

- **Goal** — the one-sentence outcome.
- **Stories** — the work items, roughly priority-ordered.
- **Acceptance Criteria** — what "done" means for the sprint.
- **Dependencies** — what must exist before the sprint starts.
- **Risks** — known blockers or uncertainties.

Stories are labeled by context: `[AUTH]`, `[PORT]`, `[INGEST]`, `[LLM]`, `[ANALYSIS]`, `[FEED]`, `[FE]`, `[INFRA]`, `[TEST]`.

---

## Phase 1 — Foundation (Sprints 1-2)

### Sprint 1: Project Scaffolding + Auth + Core Models (Weeks 1-2)

**Goal:** A running Rust backend and React frontend with passwordless auth, basic CRUD, and Docker Compose dev environment.

**Stories:**

1. `[INFRA]` Initialize Cargo workspace with 8 crates (`finima-core`, `finima-db`, `finima-api`, `finima-auth`, `finima-ingest`, `finima-llm`, `finima-analysis`, `finima-feed`). Wire `Cargo.toml` dependencies. Verify `cargo build --workspace` succeeds.

2. `[INFRA]` Create `docker-compose.yml` (dev profile) with PostgreSQL 16 and Ollama services. Create YAML config hierarchy: `config/default.yaml`, `config/development.yaml`, `config/test.yaml`, `config/production.yaml`. Create `.env.example` (secrets only: `APP__AUTH__JWT_SECRET`, `APP__RESEND__API_KEY`, `APP__DATABASE__URL`). Create root `Makefile` with `dev`, `build`, `test`, `lint`, `docker-up`, `docker-down`, `migrate` targets. See [ADR-009](../ADRs/ADR-009-externalized-yaml-configuration.md).

3. `[CORE]` Define domain models in `finima-core`: `User`, `Portfolio`, `Account`, `Transaction`, `Upload`, `RecurringGroup`, `Budget`, `SavingsGoal`, `AccountFlow`, `FlowGroup`. Define enums: `AccountType`, `Frequency`, `UploadStatus`, `FileFormat`. Define `AppError` enum with `From` impls for Axum `IntoResponse`.

4. `[DB]` Set up `finima-db`: PgPool configuration, SQLx migrations for all tables defined in the PRD data model (Section 7). Create migration files: `001_users.sql`, `002_magic_links.sql`, `003_sessions.sql`, `004_portfolios.sql`, `005_accounts.sql`, `006_transactions.sql`, `007_uploads.sql`, `008_recurring_groups.sql`, `009_budgets.sql`, `010_savings_goals.sql`, `011_user_category_overrides.sql`, `012_account_flows.sql`, `013_flow_groups.sql`. Create indexes: B-tree on FKs, GIN on `transactions.tags`, composite `(account_id, date)`, unique `(account_id, dedup_hash)`.

5. `[DB]` Implement repository traits in `finima-core` and implementations in `finima-db` for: `UserRepo`, `PortfolioRepo`, `AccountRepo`. Methods: CRUD operations, ownership checks.

6. `[AUTH]` Implement `finima-auth`: magic link token generation (32 bytes, `OsRng`), SHA-256 hashing, magic link DB operations (create, validate, mark used). JWT encode/decode with `jsonwebtoken` crate. Resend API client (`reqwest`). Auth middleware (Axum extractor `AuthUser`).

7. `[API]` Set up `finima-api`: Axum server with `AppState` (PgPool, config). Router assembly. CORS middleware (tower-http). Health check endpoint. Auth routes: `POST /auth/magic-link`, `POST /auth/verify`, `POST /auth/refresh`, `DELETE /auth/session`.

8. `[API]` Portfolio + Account routes: `GET/POST /portfolios`, `GET/PUT /portfolios/:id`, `GET/POST /accounts`, `GET/PUT/DELETE /accounts/:id`. All behind auth middleware.

9. `[FE]` Initialize frontend: Vite + React 19 + TypeScript. Install dependencies: Tailwind CSS v4, React Router v7, Zustand, React Hook Form, Zod, react-dropzone. Configure path aliases, ESLint, Prettier.

10. `[FE]` Implement `authStore` (Zustand), sign-in page (email input + "Send Magic Link" button), magic link sent confirmation page, verify page (handles callback, stores JWT). Implement `useApi` hook with JWT refresh logic.

11. `[FE]` Implement app shell: sidebar navigation (Dashboard, Accounts, Transactions, Recurring, Budget, Goals, News, Settings), header with user display name, React Router route definitions.

12. `[TEST]` Backend unit tests: `finima-auth` (token generation, hashing, JWT roundtrip, expiry validation), `finima-core` (enum parsing, error conversions). Frontend unit tests: `authStore` (login/logout state transitions).

**Acceptance Criteria:**

- `make docker-up` starts the full production stack (PostgreSQL, Ollama, MinIO, backend, frontend, Caddy).
- User can enter email, receive magic link (via Resend), click link, land on empty dashboard.
- JWT refresh works transparently.
- Portfolio and account CRUD works via API (tested with curl/Playwright).
- `cargo test --workspace --lib` passes. `npm run test` passes.

**Dependencies:** None (greenfield).

**Risks:**

- Resend API key setup. Mitigate: document clearly in README, provide test mode instructions.
- SQLx compile-time checking requires DATABASE_URL. Mitigate: set up `sqlx::offline` mode early.

---

### Sprint 2: File Upload (CSV) + Transaction List (Weeks 3-4)

**Goal:** Users can upload a CSV file, map columns, import transactions, and view them in a paginated table.

**Stories:**

1. `[INGEST]` Implement `finima-ingest` CSV parser: header detection, column-name inference (common names: "Date", "Transaction Date", "Amount", "Description", "Memo"), configurable delimiter (comma/tab).

2. `[INGEST]` Implement file type detection (`detect.rs`): extension matching + magic byte validation.

3. `[INGEST]` Implement preview generation: parse first 20 rows, return column headers + sample data + inferred mapping.

4. `[INGEST]` Implement dedup hash computation: `SHA-256(date || amount || description)`.

5. `[DB]` Implement `TransactionRepo` and `UploadRepo`: bulk insert transactions (batched `INSERT`), dedup hash uniqueness constraint handling, upload status tracking.

6. `[API]` Upload routes: `POST /uploads` (multipart file), `GET /uploads/:id/preview`, `POST /uploads/:id/confirm` (with column mapping), `GET /uploads/:id/status`.

7. `[API]` Transaction routes: `GET /transactions` (filters: date range, account, category, amount range, search text; pagination: offset/limit; sort: date, amount, description). `PUT /transactions/:id` (update category, notes, tags). `POST /transactions/bulk-update`. `GET /transactions/search` (full-text).

8. `[FE]` File upload component: drag-and-drop zone (`react-dropzone`), file type validation, progress indicator.

9. `[FE]` Column mapping modal: display preview table, dropdown selectors for date/amount/description/skip per column, auto-infer initial mapping, confirm button.

10. `[FE]` Transaction table (aggregate view): TanStack Table with sortable columns, filter bar (date range picker, account dropdown, category dropdown, search input, amount range), pagination, checkbox selection for bulk edit. Category cell is inline-editable.

11. `[FE]` Account detail view: account summary card (balance, last import, txn count), transaction table scoped to one account, "Import Transactions" button.

12. `[TEST]` Backend unit tests: CSV parser (valid files, edge cases, malformed input), dedup hash (determinism, uniqueness). Test fixtures: `sample.csv`, `sample_chase.csv`, `malformed.csv`, `empty.csv`. Integration tests: upload flow end-to-end, transaction listing with filters.

13. `[FE]` Onboarding wizard: 3-step flow (Profile -> Portfolio -> Account). Redirects new users after first sign-in.

**Acceptance Criteria:**

- User completes onboarding wizard (display name, portfolio, first account).
- User uploads a CSV, maps columns via preview UI, confirms import.
- Transactions appear in the table with correct dates, amounts, descriptions.
- Duplicate detection works: re-uploading same CSV skips already-imported rows.
- Table supports sorting, filtering by date range and account, pagination.
- Inline category edit on a transaction persists to the database.

**Dependencies:** Sprint 1 (auth, accounts, DB schema).

**Risks:**

- CSV format diversity across banks. Mitigate: test with 3+ real bank CSV exports, add column-name inference heuristics.

---

## Phase 2 — Intelligence (Sprints 3-4)

### Sprint 3: LLM Integration + Categorization (Weeks 5-6)

**Goal:** Imported transactions are automatically categorized by Gemma 4 via Ollama, with user override support.

**Stories:**

1. `[LLM]` Implement `finima-llm` client trait: `LlmClient` with `categorize_batch()` method. Implement `OllamaClient`: HTTP POST to `/api/chat` with `tools` parameter, parse tool-call responses.

2. `[LLM]` Define tool schema (`tool_defs.rs`): `categorize_transaction` with category enum, subcategory, merchant_name, confidence.

3. `[LLM]` Implement prompt templates (`prompts.rs`): system prompt for categorization, few-shot user override injection, batch transaction formatting.

4. `[LLM]` Implement `Categorizer`: batch chunking (max 20), user override pattern matching (apply before LLM), LLM call orchestration, confidence thresholding (< 0.7 = flagged), progress tracking.

5. `[API]` Wire categorization into upload flow: after `POST /uploads/:id/confirm` successfully imports transactions, queue a categorization job. Run asynchronously via `tokio::spawn`.

6. `[API]` WebSocket endpoint (`WS /api/ws`): JWT-authenticated connection, push categorization progress events. Implement user-to-connection mapping.

7. `[DB]` Implement `UserCategoryOverrideRepo`: create override, list overrides for user, pattern matching query.

8. `[API]` Category override routes: `PUT /transactions/:id` triggers override logic — prompt "Apply to all from [merchant]?" handled client-side, calls `POST /user-overrides` if confirmed.

9. `[FE]` WebSocket integration: `wsStore` (Zustand) manages connection, auto-reconnect with backoff. Display categorization progress bar after import.

10. `[FE]` Category override UX: inline dropdown with autocomplete on transaction category cell. After override, prompt modal: "Apply to all transactions from [merchant]?" Yes → create override + retroactive update.

11. `[FE]` "Needs Review" filter on transaction table: filter to `confidence < 0.7` transactions with warning icon.

12. `[FE]` Bulk edit: select multiple transactions via checkboxes → "Bulk Edit" dropdown → "Change Category" → category selector modal → apply to all selected.

13. `[INFRA]` Add Ollama to `docker-compose.yml` with GPU reservation. Document model pull: `ollama pull gemma4:26b-a4b-it-q4_K_M`.

14. `[TEST]` Backend unit tests: prompt rendering, tool-call response parsing (valid + malformed), batch chunking. Integration test with mock Ollama HTTP server (dev/test only, `APP_ENV=test`). Frontend: WebSocket store reconnection logic.

**Acceptance Criteria:**

- After CSV import, transactions are automatically categorized within 30-60 seconds.
- Low-confidence results (< 0.7) show a warning icon and appear in "Needs Review" filter.
- User can override a single transaction's category via inline edit.
- "Apply to all from [merchant]" creates a persistent override rule.
- Bulk edit updates multiple transactions in one action.
- WebSocket shows real-time categorization progress.
- If Ollama is unavailable, import still succeeds; categorization is queued for later.

**Dependencies:** Sprint 2 (upload flow, transaction table). Ollama + Gemma 4 model installed.

**Risks:**

- Gemma 4 GGUF availability and Ollama compatibility. Mitigate: test with model early, have fallback to smaller model.
- LLM response format variability. Mitigate: defensive JSON parsing, log malformed responses for debugging.

---

### Sprint 4: OFX/QFX/QIF Parsers + Recurring Detection (Weeks 7-8)

**Goal:** Support structured file formats (OFX, QFX, QIF) and detect recurring payments/income.

**Stories:**

1. `[INGEST]` Implement OFX/QFX/QBO parser: SGML/XML parsing with `quick-xml` or `roxmltree`. Extract `<STMTTRN>` elements: date (`DTPOSTED`), amount (`TRNAMT`), description (`NAME`/`MEMO`), type (`TRNTYPE`). Handle common SGML malformations (unclosed tags, missing headers).

2. `[INGEST]` Implement QIF parser: line-oriented state machine. Parse field codes: `D` (date), `T` (amount), `P` (payee), `M` (memo), `L` (category), `^` (record separator). Handle date format variations (MM/DD/YYYY, DD/MM/YYYY, MM/DD'YY).

3. `[INGEST]` Implement XLSX/XLS parser using `calamine` crate: sheet selector (if multiple sheets), column extraction, numeric date conversion (Excel serial dates → NaiveDate).

4. `[INGEST]` Update upload flow to route to the correct parser based on detected file type. OFX/QFX/QIF skip column mapping (auto-mapped); CSV/XLS show column mapping UI.

5. `[ANALYSIS]` Implement `RecurringDetector` in `finima-analysis`: group transactions by normalized merchant name, compute inter-date intervals, classify frequency (daily through annual with tolerances per PRD §4.5), compute average amount, project next expected date.

6. `[LLM]` Implement recurring enrichment: send candidate recurring groups to LLM for metadata enrichment (full merchant name, subscription/bill/income classification, estimated annual cost, confidence score).

7. `[DB]` Implement `RecurringGroupRepo`: create/update/list/confirm/dismiss operations.

8. `[API]` Recurring routes: `GET /recurring` (list confirmed + pending), `PUT /recurring/:id` (confirm, dismiss, edit frequency/amount).

9. `[FE]` Recurring payments page: two sections — Confirmed (table with merchant, amount, frequency, annual cost) and Pending Review (with confirm/dismiss buttons). Summary row: total recurring expenses/income per month/year.

10. `[FE]` Update upload flow to handle OFX/QFX/QIF: auto-detected format badge, no column mapping step, direct import with preview.

11. `[TEST]` Parser tests: valid OFX (20 txns), QFX (15 txns), QIF (10 txns), XLSX (single + multi-sheet). Malformed OFX. Edge-case QIF dates. Test fixtures for each format. Recurring detection tests: monthly/weekly/quarterly patterns, single transaction (not recurring), variable intervals.

**Acceptance Criteria:**

- User uploads OFX/QFX file → auto-parsed, no column mapping needed, transactions imported.
- User uploads QIF file → parsed correctly, categories from `L` field preserved.
- User uploads XLSX → sheet selector (if multi-sheet) → column mapping → import.
- Recurring detection runs after import and produces candidate groups.
- Recurring page shows pending items for review, confirmed items with annual cost summaries.
- LLM enriches recurring groups with merchant names and subscription/bill/income classification.

**Dependencies:** Sprint 3 (LLM client, categorization pipeline).

**Risks:**

- OFX SGML inconsistencies across banks. Mitigate: test with real-world OFX files from 5+ banks, implement lenient parser with fallbacks.
- QIF date format ambiguity (MM/DD vs DD/MM). Mitigate: heuristic detection based on value ranges.

---

## Phase 3 — Visualization (Sprints 5-6)

### Sprint 5: Dashboard + Charts + Budget (Weeks 9-10)

**Goal:** Full dashboard with financial charts, budget management, and savings goals.

**Stories:**

1. `[ANALYSIS]` Implement `NetWorthCalculator`: time-series computation across all non-archived accounts.

2. `[ANALYSIS]` Implement `CashFlowCalculator`: monthly income vs. expenses for the last 12 months.

3. `[ANALYSIS]` Implement `SpendingAnalyzer`: current month breakdown by category (amount + percentage).

4. `[ANALYSIS]` Implement `BudgetEngine`: budget vs. actual comparison, rollover logic, auto-suggest (3-month average per category).

5. `[ANALYSIS]` Implement `HealthScorer`: composite 0-100 score from savings rate, debt ratio, emergency fund months, spending trend.

6. `[DB]` Implement `BudgetRepo` and `SavingsGoalRepo`: CRUD operations.

7. `[API]` Dashboard routes: `GET /dashboard/summary`, `GET /dashboard/net-worth`, `GET /dashboard/cashflow`, `GET /dashboard/spending`.

8. `[API]` Budget routes: `GET /budgets`, `POST /budgets`, `GET /budgets/vs-actual`.

9. `[API]` Savings goal routes: `GET /savings-goals`, `POST /savings-goals`, `PUT /savings-goals/:id`.

10. `[FE]` Dashboard page: grid layout with widgets — Net Worth (Recharts line chart), Cash Flow (bar chart), Spending by Category (donut chart), Budget vs. Actual (progress bars), Upcoming Bills (list from recurring groups), Financial Health Score (gauge/progress).

11. `[FE]` Chart components: `NetWorthChart`, `CashFlowChart`, `SpendingDonut`, `BudgetProgress`. Each is a Recharts wrapper with consistent theming.

12. `[FE]` Dashboard widget interactivity: clicking a donut segment drills into filtered transaction view. Clicking "Upcoming Bills" navigates to Recurring page.

13. `[FE]` Budget page: category table with budget/spent/remaining/progress columns. Edit modal for setting limits and toggling rollover. "Auto-Suggest" button. Month navigator (prev/next).

14. `[FE]` Savings goals page: card layout with goal name, progress bar, projected completion date. "New Goal" form. Link account selector.

15. `[TEST]` Analysis unit tests: net worth with mixed account types, cash flow sign handling, budget rollover arithmetic, health score boundary cases. Frontend: chart rendering with test fixture data (dev/test only).

**Acceptance Criteria:**

- Dashboard renders 6 widgets with real data from imported transactions.
- Net worth line chart shows historical trend.
- Cash flow bars show income vs. expenses per month.
- Spending donut is interactive — click to drill into transactions.
- Budget page shows limits vs. actuals with progress bars. Over-budget categories highlighted in red.
- Auto-suggest proposes budget limits based on 3-month average.
- Savings goals show progress and projected completion.
- Financial health score computes and displays correctly.

**Dependencies:** Sprint 4 (categorized transactions, recurring detection).

**Risks:**

- Dashboard query performance with large transaction sets. Mitigate: add database indexes, consider materialized views if > 50K transactions.

---

### Sprint 6: Money Flow (Sankey, Waterfall, Outflow Ranking) (Weeks 11-12)

**Goal:** Inter-account flow detection with Sankey diagram, balance impact waterfall, and outflow ranking.

**Stories:**

1. `[ANALYSIS]` Implement `FlowDetector`: match outflows from primary accounts with inflows in other accounts (amount +-1%, date +-2 days). Create `AccountFlow` records. Handle one-sided flows.

2. `[ANALYSIS]` Implement `SankeyDataBuilder`: aggregate flows by source → target per month. Support flow group collapsing.

3. `[ANALYSIS]` Implement `OutflowRanker`: sort destination accounts by monthly outflow. Compute % of income and 3-month trend.

4. `[ANALYSIS]` Implement `WaterfallBuilder`: per primary account — starting balance + income - outflows = ending balance.

5. `[LLM]` Implement flow insight generation: detect flow trends, generate plain-language explanations (e.g., "Your Amex outflow increased 25% due to dining spending").

6. `[DB]` Implement `FlowRepo` and `FlowGroupRepo`: CRUD, queries by month/account/portfolio.

7. `[API]` Flow routes: `GET /flows`, `POST /flows` (manual link), `PUT /flows/:id` (confirm/dismiss), `DELETE /flows/:id`. `GET /flows/sankey`, `GET /flows/outflow-ranking`, `GET /flows/balance-impact`. Flow group routes: `GET/POST/PUT/DELETE /flow-groups`.

8. `[FE]` Primary income toggle: add to account edit form, explain what it does, trigger flow detection on toggle.

9. `[FE]` Money Flow page with 3 tabs: Sankey, Balance Impact, Flow Groups.

10. `[FE]` Sankey diagram: implement with `d3-sankey` or a React wrapper. Primary accounts as source nodes, destination accounts as target nodes. Band width proportional to flow volume. Click a band to drill into matching transactions.

11. `[FE]` Outflow ranking table: sorted by monthly outflow. Columns: account, type, monthly amount, % of income, trend arrow. Click row → navigate to account detail.

12. `[FE]` Balance impact waterfall chart: custom Recharts component or `d3` waterfall. Starting balance → + income → - outflows → ending balance. Month selector.

13. `[FE]` Manual flow linking: from transaction ⋮ menu → "Link as Transfer" → search matching transactions in other accounts (pre-filtered by amount/date) → create flow.

14. `[FE]` Flow groups: create/edit groups, assign flows, view collapsed Sankey.

15. `[FE]` LLM insight card below Sankey: display generated insight text for the selected month.

16. `[TEST]` Flow detection unit tests: matching pairs, no match (amount differs > 1%), no match (date differs > 2 days), one-sided flows. Sankey data correctness. Waterfall arithmetic. E2E: tag primary account → navigate to Money Flow → see Sankey.

**Acceptance Criteria:**

- User tags a checking account as primary income.
- System auto-detects inter-account flows (transfers, autopay, loan payments).
- Sankey diagram shows money flowing from income accounts to destination accounts.
- Outflow ranking table shows where money goes, sorted by volume, with trend arrows.
- Waterfall chart shows per-account balance impact (start + income - outflows = end).
- User can manually link two transactions as a transfer pair.
- User can create flow groups (e.g., "Housing Costs") that collapse in Sankey.
- LLM insight card explains flow trends in plain language.

**Dependencies:** Sprint 5 (dashboard infrastructure, account detail). Requires 2+ months of imported transactions across 2+ accounts for meaningful flow data.

**Risks:**

- Sankey/waterfall chart libraries. Recharts doesn't include these. Mitigate: evaluate `d3-sankey` + React wrapper early in the sprint. Budget time for custom SVG if needed.
- False positive flow matches. Mitigate: conservative thresholds (+-1% amount, +-2 days), user confirmation flow.

---

## Phase 4 — Polish (Sprints 7)

### Sprint 7: Theming + Preferences + News Feed + Responsive (Weeks 13-14)

**Goal:** Complete the user experience with theming, preferences, financial news, and mobile responsiveness.

**Stories:**

1. `[FE]` Theme system: CSS custom properties for colors, applied via `themeStore`. Light/dark/system mode toggle. Custom accent color picker (hex input or palette). Live preview in settings.

2. `[FE]` Dashboard layout management: `react-grid-layout` integration. Widgets are draggable and resizable. Layout persists to `users.preferences` JSONB via API. Widget toggle (show/hide) in settings.

3. `[FE]` Preferences page: tabs for Theme, Layout, General (currency, date format, fiscal month, default chart type), LLM (provider, model, endpoint, connection status indicator).

4. `[API]` User preferences route: `PUT /users/me/preferences` — partial JSONB update.

5. `[FEED]` Implement `finima-feed`: RSS/Atom fetcher using `feed-rs` crate. Fetch from configured sources. Parse articles. Store in database with dedup by URL hash.

6. `[FEED]` Implement article summarization: lazy LLM-powered 2-sentence summary on first access.

7. `[FEED]` Implement relevance scoring: heuristic based on user's account types and spending categories (v1, no LLM).

8. `[API]` Feed routes: `GET /feed` (paginated, filterable by topic), `GET /feed/:id/summary`.

9. `[FE]` News/Learn page: card grid with article title, source, date, summary, relevance badge. Topic filter tabs (budgeting, investing, taxes, credit, retirement). Click opens external URL.

10. `[FE]` Mobile responsive design: collapsible sidebar → hamburger menu at < 768px viewport. Stack dashboard widgets vertically. Table horizontal scroll on small screens. Touch-friendly tap targets.

11. `[FE]` Onboarding polish: add illustrations/icons to wizard steps, validate inputs with Zod, smooth transitions, CTA on empty states ("Import your first transactions!").

12. `[FE]` Transaction export: "Export CSV" button on transaction table, respects current filters.

13. `[TEST]` E2E tests: theme toggle (verify CSS variable change), responsive layout (375px viewport, sidebar collapse), news feed rendering. Frontend unit tests: theme store, preferences store.

**Acceptance Criteria:**

- User can switch between light/dark/system theme. Changes apply instantly without reload.
- User can set a custom accent color visible across all UI elements.
- Dashboard widgets are draggable and the layout persists across sessions.
- Preferences (currency, date format) are reflected throughout the UI.
- News page shows articles with LLM summaries and relevance badges.
- App is usable on mobile (375px viewport): sidebar collapses, tables scroll, charts resize.
- Transaction export produces a valid CSV file matching current filters.

**Dependencies:** Sprints 5-6 (dashboard, all core features).

**Risks:**

- `react-grid-layout` drag-and-drop UX may conflict with mobile touch gestures. Mitigate: disable drag on mobile, use simple stacked layout instead.

---

## Phase 5 — Deployment + Hardening (Sprint 8)

### Sprint 8: Docker Prod, CI/CD, Security Audit, Documentation (Weeks 15-16)

**Goal:** Production-ready deployment with automated CI/CD, security hardening, and user documentation.

**Stories:**

1. `[INFRA]` Finalize `Dockerfile.backend` (multi-stage Rust build) and `Dockerfile.frontend` (multi-stage Node build + nginx prod serve).

2. `[INFRA]` Create `docker-compose.prod.yml` with production settings: `APP_ENV=production`, secrets from `.env` (env vars override YAML), `config/production.yaml` mounted read-only, `config/frontend-prod.yaml` mounted into nginx. Caddy reverse proxy with automatic HTTPS, restart policies, health checks. **No seed data, no mock services, no test fixtures in production image** (enforced via `.dockerignore` excluding `tests/`, `config/test.yaml`).

3. `[INFRA]` GitHub Actions CI workflow (`.github/workflows/ci.yml`): backend lint (fmt + clippy), backend tests (unit + integration with Postgres service), frontend lint (ESLint + tsc), frontend tests (Vitest), E2E tests (Playwright with backend + Postgres).

4. `[INFRA]` GitHub Actions release workflow (`.github/workflows/release.yml`): triggered on version tag, builds and pushes Docker images to GHCR, creates GitHub Release with auto-generated notes.

5. `[SECURITY]` Rate limiting: tower-http rate limit on `POST /auth/magic-link` (5/hour/email). Rate limit on upload endpoint (10/hour/user). Rate limit on LLM endpoints.

6. `[SECURITY]` Input validation audit: verify all user inputs are validated (UUID format, enum membership, decimal bounds, string length limits, file size limits). Verify file type validation uses magic bytes not just extension.

7. `[SECURITY]` JWT hardening: verify short-lived access tokens (15 min), single-use refresh token rotation, secure cookie flags if applicable.

8. `[SECURITY]` CORS audit: verify strict origin whitelist. CSP headers for frontend.

9. `[SECURITY]` Database encryption: document `pgcrypto` setup for sensitive fields. Verify no plaintext secrets in logs.

10. `[TEST]` Final test pass: run full test suite (unit + integration + E2E). Fix any failures. Verify coverage meets thresholds (backend > 70%, frontend > 60%).

11. `[TEST]` Performance test: CSV import 10K rows (< 15s), dashboard query with 50K transactions (< 500ms), LLM batch 20 transactions (< 30s). Document results.

12. `[TEST]` Create `tests/seed.sql` with comprehensive test data (dev/test only, excluded from production Docker image via `.dockerignore`): 1 user, 1 portfolio, 3 accounts, 500 transactions, 5 recurring groups, 8 flow records, 1 flow group, 3 budgets, 1 savings goal. Add CI check to verify production image does not contain `seed.sql` or test fixtures.

13. `[DOCS]` Write `README.md`: project overview, quick start, development setup, deployment instructions.

14. `[DOCS]` Deployment guide: self-hosted setup step-by-step, environment variables reference, model installation, backup strategy.

**Acceptance Criteria:**

- `make docker-up` starts a fully functional production deployment.
- CI pipeline runs on every PR: lint, test, E2E. Blocks merge on failure.
- Release workflow builds and pushes Docker images on version tag.
- All security measures in place: rate limiting, input validation, JWT rotation, CORS, magic bytes validation.
- Performance benchmarks meet PRD targets.
- README and deployment docs are complete and accurate.
- A new user can follow the README to deploy Finima on a fresh server.

**Dependencies:** All previous sprints.

**Risks:**

- E2E test flakiness in CI. Mitigate: retry logic, headless browser stability, generous timeouts.
- Docker image size (Rust binaries can be large). Mitigate: multi-stage build with slim base, strip debug symbols.

---

## Sprint Summary

| Sprint | Weeks | Phase         | Focus                      | Key Deliverables                                                              |
| ------ | ----- | ------------- | -------------------------- | ----------------------------------------------------------------------------- |
| 1      | 1-2   | Foundation    | Scaffolding + Auth + CRUD  | Workspace, Docker, Auth flow, Portfolio/Account CRUD, App shell               |
| 2      | 3-4   | Foundation    | File Upload + Transactions | CSV import, Column mapping, Transaction table, Onboarding wizard              |
| 3      | 5-6   | Intelligence  | LLM + Categorization       | Gemma 4 integration, Auto-categorization, User overrides, WebSocket           |
| 4      | 7-8   | Intelligence  | Parsers + Recurring        | OFX/QFX/QIF/XLSX parsers, Recurring detection, LLM enrichment                 |
| 5      | 9-10  | Visualization | Dashboard + Budget         | Charts, Dashboard widgets, Budget management, Savings goals, Health score     |
| 6      | 11-12 | Visualization | Money Flow                 | Sankey diagram, Waterfall chart, Outflow ranking, Flow groups, LLM insights   |
| 7      | 13-14 | Polish        | Theming + News + Mobile    | Theme system, Drag-drop layout, News feed, Responsive design                  |
| 8      | 15-16 | Deployment    | CI/CD + Security + Docs    | Docker prod, GitHub Actions, Security audit, Performance tests, Documentation |

---

## Cross-Cutting Concerns (Every Sprint)

- **Configuration:** All configuration is externalized into YAML files (`config/default.yaml`, `config/{APP_ENV}.yaml`). Secrets are injected via environment variables. No hardcoded URLs, keys, or feature flags in source code. Frontend reads runtime config from `/config.yaml` at startup. See [ADR-009](../ADRs/ADR-009-externalized-yaml-configuration.md).
- **Environment discipline:** Three profiles — `development`, `test`, `production`. Mock services, test doubles, and seed data exist **only in `development` and `test`**. Production connects exclusively to real services and starts with an empty database. CI verifies that production Docker images contain no test fixtures, seed scripts, or mock endpoints.
- **Testing:** Every story includes unit tests. Integration tests for API endpoints. E2E tests for completed user flows. All mocks and seed data scoped to `APP_ENV=test`.
- **Logging:** Structured logging with `tracing` (backend) from Sprint 1. Log level and format configured via YAML (`logging.level`, `logging.format`). Frontend error boundaries from Sprint 1.
- **Error handling:** `AppError` enum with proper HTTP status codes. User-facing error messages are helpful, not leaky.
- **Accessibility:** Semantic HTML, ARIA labels on interactive elements, keyboard navigation for tables and forms.
- **Git workflow:** Feature branches, PR reviews, squash merge to main. Conventional commits.

---

## Definition of Done (Per Story)

1. Code written and compiles without warnings.
2. Unit tests pass for the new/modified code.
3. Integration test exists for API endpoints.
4. No `clippy` warnings, no ESLint errors, no TypeScript errors.
5. Manually tested in browser (for frontend stories).
6. PR reviewed and merged.

---

## Key Technical Decisions to Make Early

| Decision Point                  | Sprint                       | Options                                                    | Recommendation                                         |
| ------------------------------- | ---------------------------- | ---------------------------------------------------------- | ------------------------------------------------------ |
| Sankey chart library            | Sprint 6 (spike in Sprint 5) | `d3-sankey` + React wrapper, `recharts-sankey`, custom SVG | Spike `d3-sankey` in Sprint 5; fall back to custom SVG |
| Waterfall chart                 | Sprint 6                     | Custom Recharts shapes, `d3` waterfall, `nivo`             | Custom Recharts `Bar` with stacking                    |
| LLM client: Ollama vs llama.cpp | Sprint 3                     | Ollama HTTP (simpler) vs llama-cpp-4 (lower latency)       | Start with Ollama; add llama.cpp as optional feature   |
| sqlx offline mode               | Sprint 1                     | Always online DB vs offline mode for CI                    | Set up offline mode immediately for CI reliability     |
| Configuration format            | Sprint 1                     | `.env` (dotenvy), YAML (config-rs), TOML                   | YAML via config-rs with layered profiles (ADR-009)     |
| Frontend runtime config         | Sprint 1                     | Vite `VITE_*` env vars, runtime YAML fetch                 | Runtime YAML fetch at startup (`/config.yaml`)         |
| CSS framework                   | Sprint 1                     | Tailwind v4, Tailwind v3, CSS Modules                      | Tailwind v4 (latest, CSS-first config)                 |
