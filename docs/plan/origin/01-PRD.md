# Finima — Product Requirements Document (PRD)

**Version:** 1.0  
**Date:** 2026-04-10  
**Author:** Chris Phillipson  
**Status:** Draft

---

## 1. Executive Summary

Finima is a privacy-first, AI-augmented personal finance platform that gives individuals and households complete visibility into their financial health — without surrendering data to third-party aggregators. Users import their own bank extracts (CSV, OFX, QFX, QIF, XLS/XLSX), and a locally-hosted LLM (Gemma 4 via llama.cpp) handles transaction categorization, recurring-payment detection, and spending insights through structured tool calling.

The system is composed of a Rust multi-crate backend serving a REST/WebSocket API, and a modern TypeScript (React + Vite) frontend with theming, layout management, and user preferences. Authentication is passwordless via email magic links powered by Resend.

### Competitive Inspiration

Finima draws from the strengths of leading personal finance tools while charting its own course:

- **Monarch Money** — visual dashboards, net-worth tracking, household sharing, AI assistant, and receipt scanning ([monarchmoney.com](https://www.monarch.com))
- **YNAB (You Need A Budget)** — zero-based budgeting philosophy, every-dollar assignment, strong community education ([ynab.com](https://www.ynab.com))
- **Copilot** — clean mobile-first UI, smart categorization, subscription detection ([copilot.money](https://copilot.money))
- **Quicken Simplifi** — spending plans, investment tracking, bill management ([quicken.com/simplifi](https://www.quicken.com/simplifi))
- **PocketSmith** — calendar-based cash flow forecasting, multi-currency, CSV/OFX/QFX/QIF import support ([pocketsmith.com](https://www.pocketsmith.com))

Finima differentiates by running its AI entirely on-device or on self-hosted infrastructure (no cloud LLM dependency), supporting direct file import without Plaid/aggregator lock-in, and being fully open-source.

---

## 2. Problem Statement

Existing personal finance apps suffer from one or more of:

1. **Aggregator dependency** — Plaid, Yodlee, or Finicity connections break frequently, expose credentials, and limit bank coverage internationally.
2. **Cloud-only AI** — Monarch's AI assistant and Copilot's "Intelligence" engine send financial data to proprietary cloud APIs.
3. **Limited import flexibility** — Many apps poorly handle CSV variants or ignore OFX/QFX/QIF entirely, requiring manual reformatting.
4. **Weak categorization** — Rule-based systems require extensive manual setup; ML-based systems are opaque black boxes with no user override path.
5. **No local-first option** — Users who want complete data sovereignty have no polished alternative.

---

## 3. Target Users

| Persona               | Description                                                                                                                           |
| --------------------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| **DIY Budgeter**      | Wants to track spending without connecting bank credentials to third parties. Comfortable downloading CSV/OFX from their bank portal. |
| **Household Manager** | Manages 5–15 accounts across checking, savings, credit, loans, and investments for a family. Needs aggregate views.                   |
| **Privacy Advocate**  | Wants all data and AI processing to stay local or self-hosted. Zero cloud dependency for core features.                               |
| **Financial Learner** | Newer to personal finance; benefits from AI-driven insights, budget suggestions, and educational financial news.                      |

---

## 4. Core Features

### 4.1 Authentication & Onboarding

- **Passwordless auth** via email magic links using the Resend API.
- Flow: user enters email → backend generates a time-limited token (expires 15 min) → Resend delivers the email → user clicks link → backend validates token, issues JWT session + refresh token.
- New users land on a guided onboarding wizard: create profile → create first portfolio → add first account → upload first file.

### 4.2 Portfolio & Account Management

- A **Portfolio** is the top-level container (e.g., "Phillipson Household").
- Portfolios contain one or more **Accounts**.
- Account types: `checking`, `savings`, `credit_card`, `investment_brokerage`, `investment_retirement`, `loan_mortgage`, `loan_auto`, `loan_student`, `loan_personal`, `cash`, `crypto`, `other`.
- Each account stores: name, institution, account type, currency (default USD), opening balance, current balance (computed), notes.
- Accounts can be archived (soft-delete) but never hard-deleted while transactions exist.

### 4.3 File Upload & Parsing

Supported formats (ranked by parsing reliability, per industry consensus — OFX/QFX preferred, CSV as fallback):

| Format                     | Extension(s)           | Parser Strategy                                                      |
| -------------------------- | ---------------------- | -------------------------------------------------------------------- |
| Open Financial Exchange    | `.ofx`, `.qfx`, `.qbo` | XML/SGML structured parse; extract `<STMTTRN>` elements              |
| Quicken Interchange Format | `.qif`                 | Line-oriented parser, `D`/`T`/`P`/`M`/`L` field codes                |
| CSV / TSV                  | `.csv`, `.tsv`         | Column-mapping wizard; user confirms date/amount/description columns |
| Excel                      | `.xls`, `.xlsx`        | Sheet selector → column-mapping wizard (same as CSV)                 |

Upload flow:

1. User selects target account.
2. Drags/drops or picks file(s).
3. Backend parses and returns a preview (first 20 rows + inferred columns).
4. User confirms column mapping (for CSV/XLS) or accepts auto-detected fields (for OFX/QFX/QIF).
5. Backend inserts transactions, runs duplicate detection (date + amount + description hash), and queues an LLM categorization job.

### 4.4 LLM-Powered Categorization & Classification

The backend hosts a Gemma 4 model (recommended: `gemma-4-26B-A4B-it` Q4 GGUF via Ollama, or `gemma-4-E4B-it` for resource-constrained setups) through llama.cpp Rust bindings (`llama-cpp-4` crate) or via Ollama's OpenAI-compatible `/api/chat` endpoint.

Categorization uses Gemma 4's native function-calling capability with structured JSON output:

**Tool definition provided to the model:**

```json
{
  "name": "categorize_transaction",
  "description": "Assign a category and subcategory to a financial transaction",
  "parameters": {
    "type": "object",
    "properties": {
      "category": {
        "type": "string",
        "enum": [
          "housing",
          "transportation",
          "food_dining",
          "utilities",
          "healthcare",
          "insurance",
          "entertainment",
          "shopping",
          "personal_care",
          "education",
          "travel",
          "gifts_donations",
          "income",
          "transfer",
          "fees_charges",
          "investment",
          "debt_payment",
          "other"
        ]
      },
      "subcategory": { "type": "string" },
      "merchant_name": { "type": "string" },
      "confidence": { "type": "number", "minimum": 0, "maximum": 1 }
    },
    "required": ["category", "subcategory", "merchant_name", "confidence"]
  }
}
```

- Transactions are batched (up to 20 per LLM call) for throughput.
- Results with `confidence < 0.7` are flagged for user review.
- Users can override single transactions or bulk-select and re-categorize.
- Overrides are stored and fed back as few-shot examples in subsequent categorization prompts (user-specific learning).

### 4.5 Recurring Payment / Credit Detection

A dedicated analysis pipeline identifies repeating transactions:

1. **Candidate grouping** — cluster transactions by normalized merchant name (LLM-enriched) and amount (±5% tolerance for variable amounts).
2. **Frequency detection** — analyze date intervals within each cluster:
   - Daily (1 day ± 0)
   - Weekly (7 days ± 1)
   - Biweekly (14 days ± 2)
   - Monthly (28–31 days ± 3)
   - Quarterly (85–95 days ± 5)
   - Semi-annually (175–190 days ± 10)
   - Annually (355–375 days ± 15)
   - Variable (detected but no consistent period)
3. **Metadata enrichment** — for each detected recurring item, the LLM enriches with: merchant full name, category, estimated annual cost, whether it's a subscription/bill/income, and a confidence score.
4. **User confirmation** — detected recurrences are presented for review. Users can confirm, dismiss, or adjust frequency/amount.

### 4.6 Dashboard & Financial Health

The main dashboard provides at-a-glance financial health:

- **Net Worth** — sum of all account balances over time (line chart).
- **Cash Flow** — income vs. expenses per month (bar chart).
- **Spending by Category** — current month breakdown (donut/treemap chart).
- **Budget vs. Actual** — progress bars per category.
- **Upcoming Bills** — next 30 days of predicted recurring payments.
- **Financial Health Score** — composite metric (savings rate, debt-to-income, emergency fund months, spending trend).

### 4.7 Aggregate & Detail Views

- **Aggregate view** — all accounts combined, filterable by date range, category, account type. Tables with sortable columns. Charts for trends.
- **Detail view** — per-account transaction list with search, filter, sort. Inline editing of category, notes, tags. Split transaction support.
- **Charts available** — line (trends), bar (comparisons), donut (proportions), area (cumulative), heatmap (spending by day-of-week/month).

### 4.8 Budget & Savings Planning

- Users set monthly budget targets per category (or use LLM-suggested defaults based on historical spending).
- Rollover support: unspent budget can roll to next month (configurable).
- Savings goals: name, target amount, target date, linked account. Progress tracking with projected completion date.
- Household budget mode: combines all accounts in portfolio.

### 4.9 Financial News & Investment Literacy Feed

- Curated RSS/Atom feed aggregation from reputable sources (configurable list).
- Default sources: Investopedia, NerdWallet, The Motley Fool educational content, Federal Reserve economic data summaries.
- LLM-powered summarization: each article gets a 2-sentence summary and a relevance score based on user's portfolio composition.
- Investment literacy section: glossary, beginner guides, contextual tips surfaced based on user's account types.

### 4.10 Preferences, Theming & Layout

- **Theme system** — light/dark/auto modes with customizable accent color. CSS custom properties propagated from a theme config.
- **Layout management** — dashboard widgets are drag-and-drop rearrangeable (grid layout). Users save their preferred layout.
- **Preferences** — currency display, date format, fiscal month start, default chart type, notification settings, LLM model selection (if multiple available).

### 4.11 Account Flow Mapping & Inter-Account Analysis

Users can designate one or more accounts as **primary income accounts** — the accounts where paychecks or regular income land. Once tagged, Finima builds an inter-account flow graph that answers: "Where does my paycheck go each month?"

**Primary account tagging:**

- Any account can be flagged `is_primary_income = true` via account settings.
- A portfolio may have multiple primary accounts (e.g., a household with two earners depositing into different checking accounts).

**Transfer detection & linking:**

- The system identifies inter-account transfers by matching complementary transactions across accounts: outflow from Account A on date D for amount X, paired with inflow to Account B on date D (±2 days) for amount X (±1% tolerance).
- Matched pairs are stored as `account_flows` records linking `source_account_id → target_account_id` with amount, date, and whether the match was auto-detected or user-confirmed.
- LLM-assisted: descriptions like "TRANSFER TO SAVINGS", "AUTOPAY AMEX", "MORTGAGE PMT" are analyzed to infer the target account when only one side of the transfer is visible (common when the user hasn't imported statements from all accounts).

**Flow visualization:**

- **Sankey diagram** — a primary dashboard widget showing money flowing from income accounts → checking → savings, credit cards, loans, investments. Width of each flow band is proportional to monthly volume.
- **Outflow ranking table** — sorted list of destination accounts by total monthly outflow from primary accounts. Columns: account name, type, avg monthly outflow, % of income, trend (↑↓→).
- **Balance impact chart** — for each primary account, a waterfall chart showing: starting balance + income − outflows to each linked account = ending balance. Makes it immediately visible which credit card or loan is consuming the most of each paycheck.

**Monthly flow summary:**

- Per-month breakdown: "Of your $8,200 income, $1,800 went to rent (mortgage), $650 to Amex autopay, $500 to savings transfer, $120 to student loan..."
- Trend analysis: "Your Amex outflow increased 18% over the last 3 months."
- LLM insight: when the model detects a flow increase, it can generate a plain-language explanation: "Your credit card payments increased because your dining and entertainment spending rose from $400 to $620."

**User controls:**

- Manual flow linking: user can explicitly link two transactions as a transfer pair when auto-detection misses.
- Flow dismissal: user can mark a detected flow as "not a transfer" (e.g., a coincidental same-amount transaction).
- Flow grouping: user can name flow groups (e.g., "Housing costs" = mortgage + property tax + insurance outflows).

---

## 5. Non-Functional Requirements

| Requirement                          | Target                                              |
| ------------------------------------ | --------------------------------------------------- |
| Cold start (backend)                 | < 10s including model load                          |
| API response (non-LLM)               | < 200ms p95                                         |
| LLM categorization batch (20 txns)   | < 30s                                               |
| File upload parse (10K transactions) | < 15s                                               |
| Frontend initial load (cached)       | < 2s                                                |
| Data at rest encryption              | AES-256 (SQLite + SQLCipher or PostgreSQL pgcrypto) |
| Auth token expiry                    | Access: 15 min, Refresh: 7 days                     |
| Concurrent users (self-hosted)       | 1–10 (household scale)                              |

---

## 6. Technology Stack

### Backend (Rust, multi-crate workspace)

| Crate             | Responsibility                                                                                           |
| ----------------- | -------------------------------------------------------------------------------------------------------- |
| `finima-core`     | Domain models, business logic, error types                                                               |
| `finima-db`       | Database layer (SQLx + PostgreSQL), migrations, queries                                                  |
| `finima-api`      | Axum HTTP server, REST routes, WebSocket handlers                                                        |
| `finima-auth`     | Magic link token generation/validation, JWT, Resend integration                                          |
| `finima-ingest`   | File parsers (OFX, QFX, QIF, CSV, XLS), column mapping, dedup                                            |
| `finima-llm`      | LLM client abstraction (llama.cpp bindings or Ollama HTTP), tool-calling orchestration, prompt templates |
| `finima-analysis` | Recurring detection, budget computation, financial health scoring                                        |
| `finima-feed`     | RSS/Atom fetcher, article summarization queue                                                            |

### Frontend (TypeScript)

- **Framework**: React 19 + Vite
- **State**: Zustand (lightweight, no boilerplate)
- **Routing**: React Router v7
- **Styling**: Tailwind CSS v4 + CSS custom properties for theming
- **Charts**: Recharts (React-native charting, already available in artifact env)
- **Tables**: TanStack Table v8
- **Drag/drop layout**: `react-grid-layout`
- **File upload**: `react-dropzone`
- **Forms**: React Hook Form + Zod validation
- **HTTP**: Fetch API + custom hooks (no Axios dependency)

### Infrastructure

- **Database**: PostgreSQL 16
- **LLM runtime**: Ollama (preferred for ease) or llama.cpp server
- **Email**: Resend API
- **Containerization**: Docker Compose (dev/prod profiles)
- **CI/CD**: GitHub Actions
- **Build**: Makefiles for frontend and backend

---

## 7. Data Model (Simplified)

```text
users
  id: UUID (PK)
  email: TEXT (unique)
  display_name: TEXT
  preferences: JSONB
  created_at: TIMESTAMPTZ
  updated_at: TIMESTAMPTZ

sessions
  id: UUID (PK)
  user_id: UUID (FK → users)
  token_hash: TEXT
  expires_at: TIMESTAMPTZ

magic_links
  id: UUID (PK)
  email: TEXT
  token_hash: TEXT
  expires_at: TIMESTAMPTZ
  used_at: TIMESTAMPTZ?

portfolios
  id: UUID (PK)
  user_id: UUID (FK → users)
  name: TEXT
  created_at: TIMESTAMPTZ

accounts
  id: UUID (PK)
  portfolio_id: UUID (FK → portfolios)
  name: TEXT
  institution: TEXT?
  account_type: TEXT (enum)
  currency: TEXT (default 'USD')
  opening_balance: DECIMAL
  is_primary_income: BOOL (default false)
  is_archived: BOOL
  created_at: TIMESTAMPTZ

transactions
  id: UUID (PK)
  account_id: UUID (FK → accounts)
  date: DATE
  amount: DECIMAL
  description: TEXT
  original_description: TEXT
  category: TEXT?
  subcategory: TEXT?
  merchant_name: TEXT?
  tags: TEXT[]
  notes: TEXT?
  is_recurring: BOOL
  recurring_group_id: UUID?
  llm_confidence: FLOAT?
  user_overridden: BOOL
  dedup_hash: TEXT
  created_at: TIMESTAMPTZ

uploads
  id: UUID (PK)
  account_id: UUID (FK → accounts)
  filename: TEXT
  format: TEXT
  row_count: INT
  status: TEXT (pending | processing | complete | error)
  error_message: TEXT?
  uploaded_at: TIMESTAMPTZ

recurring_groups
  id: UUID (PK)
  portfolio_id: UUID (FK → portfolios)
  merchant_name: TEXT
  category: TEXT
  frequency: TEXT (daily|weekly|biweekly|monthly|quarterly|semiannual|annual|variable)
  avg_amount: DECIMAL
  is_confirmed: BOOL
  next_expected_date: DATE?
  metadata: JSONB

budgets
  id: UUID (PK)
  portfolio_id: UUID (FK → portfolios)
  category: TEXT
  monthly_limit: DECIMAL
  rollover: BOOL
  month: DATE (first of month)

savings_goals
  id: UUID (PK)
  portfolio_id: UUID (FK → portfolios)
  name: TEXT
  target_amount: DECIMAL
  current_amount: DECIMAL
  target_date: DATE?
  linked_account_id: UUID?

user_category_overrides
  id: UUID (PK)
  user_id: UUID (FK → users)
  description_pattern: TEXT
  category: TEXT
  subcategory: TEXT

account_flows
  id: UUID (PK)
  portfolio_id: UUID (FK → portfolios)
  source_account_id: UUID (FK → accounts)
  target_account_id: UUID (FK → accounts)
  source_transaction_id: UUID? (FK → transactions)
  target_transaction_id: UUID? (FK → transactions)
  amount: DECIMAL
  flow_date: DATE
  is_auto_detected: BOOL
  is_confirmed: BOOL
  flow_group_id: UUID? (FK → flow_groups)
  created_at: TIMESTAMPTZ

flow_groups
  id: UUID (PK)
  portfolio_id: UUID (FK → portfolios)
  name: TEXT (e.g., "Housing Costs", "Debt Payments")
  created_at: TIMESTAMPTZ
```

---

## 8. API Surface (Key Endpoints)

```text
POST   /api/auth/magic-link          Send magic link email
POST   /api/auth/verify              Verify token, issue JWT
POST   /api/auth/refresh             Refresh access token
DELETE /api/auth/session              Logout

GET    /api/portfolios               List user's portfolios
POST   /api/portfolios               Create portfolio
GET    /api/portfolios/:id           Get portfolio detail
PUT    /api/portfolios/:id           Update portfolio

GET    /api/accounts                 List accounts (query by portfolio)
POST   /api/accounts                 Create account
GET    /api/accounts/:id             Get account detail + balance
PUT    /api/accounts/:id             Update account
DELETE /api/accounts/:id             Archive account

POST   /api/uploads                  Upload file (multipart)
GET    /api/uploads/:id/preview      Preview parsed rows
POST   /api/uploads/:id/confirm      Confirm column mapping, start import
GET    /api/uploads/:id/status       Poll import status

GET    /api/transactions             List transactions (filters, pagination, sort)
PUT    /api/transactions/:id         Update transaction (category override)
POST   /api/transactions/bulk-update Bulk category update
GET    /api/transactions/search      Full-text search

GET    /api/recurring                List detected recurring items
PUT    /api/recurring/:id            Confirm/dismiss/edit recurring item

GET    /api/dashboard/summary        Dashboard aggregates
GET    /api/dashboard/net-worth      Net worth time series
GET    /api/dashboard/cashflow       Cash flow by month
GET    /api/dashboard/spending       Spending by category

GET    /api/budgets                  List budgets for current month
POST   /api/budgets                  Set/update budget
GET    /api/budgets/vs-actual        Budget vs actual comparison

GET    /api/savings-goals            List savings goals
POST   /api/savings-goals            Create goal
PUT    /api/savings-goals/:id        Update goal

GET    /api/feed                     Financial news feed (paginated)
GET    /api/feed/:id/summary         LLM summary for article

GET    /api/flows                    List inter-account flows (query by month, account)
POST   /api/flows                    Manually link two transactions as a transfer pair
PUT    /api/flows/:id                Confirm/dismiss/edit a detected flow
DELETE /api/flows/:id                Remove a flow link
GET    /api/flows/sankey             Sankey diagram data (source→target→amount per month)
GET    /api/flows/outflow-ranking    Outflow ranking from primary accounts (sorted by volume)
GET    /api/flows/balance-impact     Waterfall data: income − outflows per primary account
GET    /api/flow-groups              List user-defined flow groups
POST   /api/flow-groups              Create flow group (e.g., "Housing Costs")
PUT    /api/flow-groups/:id          Update flow group (rename, add/remove flows)
DELETE /api/flow-groups/:id          Delete flow group

WS     /api/ws                       WebSocket for real-time updates (import progress, LLM status)
```

---

## 9. Milestones

| Phase                       | Scope                                                                               | Duration |
| --------------------------- | ----------------------------------------------------------------------------------- | -------- |
| **Phase 1 — Foundation**    | Auth, portfolio/account CRUD, file upload (CSV only), basic transaction list        | 4 weeks  |
| **Phase 2 — Intelligence**  | LLM integration, categorization, OFX/QFX/QIF parsers, recurring detection           | 4 weeks  |
| **Phase 3 — Visualization** | Dashboard, charts, aggregate views, budget management                               | 3 weeks  |
| **Phase 4 — Polish**        | Theming/preferences, savings goals, news feed, onboarding wizard, mobile responsive | 3 weeks  |
| **Phase 5 — Deployment**    | Docker Compose prod profile, GitHub Actions CI/CD, documentation, security audit    | 2 weeks  |

---

## 10. Success Metrics

- User can go from sign-up to viewing categorized transactions in < 10 minutes.
- LLM categorization accuracy > 80% (measured by user override rate < 20%).
- Recurring detection precision > 85%.
- Dashboard loads in < 3 seconds with 10K transactions across 5 accounts.
- Zero third-party data sharing for core functionality.

---

## 11. References

- Gemma 4 model card and function calling: [ai.google.dev/gemma/docs/core/model_card_4](https://ai.google.dev/gemma/docs/core/model_card_4)
- Gemma 4 announcement (Hugging Face): [huggingface.co/blog/gemma4](https://huggingface.co/blog/gemma4)
- Gemma 4 function calling guide: [ai.google.dev/gemma/docs/capabilities/text/function-calling-gemma4](https://ai.google.dev/gemma/docs/capabilities/text/function-calling-gemma4)
- llama-cpp-4 Rust crate: [crates.io/crates/llama-cpp-4](https://crates.io/crates/llama-cpp-4)
- utilityai/llama-cpp-rs (tool-calling support): [github.com/utilityai/llama-cpp-rs](https://github.com/utilityai/llama-cpp-rs)
- OFX file format specification: [openbankingtracker.com/standards/ofx](https://www.openbankingtracker.com/standards/ofx)
- Financial file format overview: [QuickBooks Community](https://quickbooks.intuit.com/learn-support/en-us/talk-about-your-business/financial-file-formats/00/174212)
- PocketSmith file format support: [learn.pocketsmith.com/article/145](https://learn.pocketsmith.com/article/145-preparing-your-bank-files-accepted-file-types)
- Resend email API: [resend.com](https://resend.com/)
- Auth.js + Resend magic links: [authjs.dev/guides/configuring-resend](https://authjs.dev/guides/configuring-resend)
- Monarch Money features: [monarch.com](https://www.monarch.com)
- YNAB budgeting philosophy: [ynab.com](https://www.ynab.com)
- NerdWallet best budget apps 2026: [nerdwallet.com](https://www.nerdwallet.com/finance/learn/best-budget-apps)
- Gemma 4 agent building guide: [lushbinary.com](https://lushbinary.com/blog/build-ai-agent-gemma-4-function-calling-mcp-tool-use/)
