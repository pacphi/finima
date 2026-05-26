# Finima — System Architecture

**Version:** 1.0 | **Date:** 2026-04-10

---

## 1. High-Level Architecture

```text
┌─────────────────────────────────────────────────────────────────────┐
│                         CLIENT (Browser)                             │
│                                                                      │
│  React 19 + Vite + Zustand + Tailwind CSS + Recharts                │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐               │
│  │  Auth    │ │Dashboard │ │ Transact.│ │ Settings │               │
│  │  Pages   │ │  Views   │ │  Views   │ │  Views   │               │
│  └────┬─────┘ └────┬─────┘ └────┬─────┘ └────┬─────┘               │
│       │             │            │             │                      │
│       └─────────────┴────────────┴─────────────┘                     │
│                         │ HTTP/REST + WebSocket                       │
└─────────────────────────┼───────────────────────────────────────────┘
                          │
┌─────────────────────────┼───────────────────────────────────────────┐
│                   BACKEND (Rust / Axum)                               │
│                         │                                            │
│  ┌──────────────────────┴──────────────────────────────────────┐    │
│  │                     finima-api (Axum)                         │    │
│  │  Routes · Middleware (JWT auth, rate limit, CORS) · WebSocket │    │
│  └──────┬──────────┬──────────┬──────────┬──────────┬──────────┘    │
│         │          │          │          │          │                 │
│  ┌──────┴───┐ ┌────┴────┐ ┌──┴──────┐ ┌┴────────┐ ┌┴──────────┐   │
│  │finima-   │ │finima-  │ │finima-  │ │finima-  │ │finima-    │   │
│  │auth      │ │ingest   │ │llm      │ │analysis │ │feed       │   │
│  │          │ │         │ │         │ │         │ │           │   │
│  │Magic link│ │OFX/QFX/ │ │Ollama or│ │Recurring│ │RSS/Atom   │   │
│  │JWT/Resend│ │QIF/CSV/ │ │llama.cpp│ │Budget   │ │fetch +    │   │
│  │          │ │XLS parse│ │tool call│ │Health   │ │summarize  │   │
│  └──────┬───┘ └────┬────┘ └──┬──────┘ └┬────────┘ └┬──────────┘   │
│         │          │         │         │           │                │
│  ┌──────┴──────────┴─────────┴─────────┴───────────┴──────────┐    │
│  │                     finima-core                              │    │
│  │  Domain models · Business rules · Error types · Traits       │    │
│  └──────────────────────────┬──────────────────────────────────┘    │
│                              │                                      │
│  ┌──────────────────────────┴──────────────────────────────────┐    │
│  │                     finima-db (SQLx)                          │    │
│  │  Migrations · Queries · Connection pool · PostgreSQL         │    │
│  └──────────────────────────┬──────────────────────────────────┘    │
│                              │                                      │
└──────────────────────────────┼──────────────────────────────────────┘
                               │
          ┌────────────────────┼────────────────────┐
          │                    │                    │
    ┌─────┴─────┐      ┌──────┴──────┐     ┌──────┴──────┐
    │PostgreSQL │      │ Ollama /    │     │  Resend     │
    │   16      │      │ llama.cpp   │     │  API        │
    │           │      │ server      │     │  (email)    │
    └───────────┘      └─────────────┘     └─────────────┘
```

---

## 2. Backend Crate Workspace

```text
finima/
├── Cargo.toml                    # Workspace manifest
├── Makefile                      # Build, test, lint, run targets
├── crates/
│   ├── finima-core/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── models/           # User, Portfolio, Account, Transaction, etc.
│   │       ├── errors.rs         # AppError enum, From impls for Axum
│   │       ├── types.rs          # AccountType, Frequency, etc. enums
│   │       └── traits.rs         # Repository traits (for testability)
│   │
│   ├── finima-db/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── pool.rs           # PgPool setup
│   │       ├── migrations/       # SQLx migrations (sequential .sql files)
│   │       ├── repos/            # Impl of core::traits for each entity
│   │       │   ├── user_repo.rs
│   │       │   ├── portfolio_repo.rs
│   │       │   ├── account_repo.rs
│   │       │   ├── transaction_repo.rs
│   │       │   ├── upload_repo.rs
│   │       │   ├── recurring_repo.rs
│   │       │   ├── budget_repo.rs
│   │       │   ├── savings_goal_repo.rs
│   │       │   └── flow_repo.rs
│   │       └── queries/          # Raw SQL or SQLx macros
│   │
│   ├── finima-auth/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── magic_link.rs     # Token generation, hashing, validation
│   │       ├── jwt.rs            # JWT encode/decode, claims struct
│   │       ├── resend.rs         # Resend API client (reqwest + serde)
│   │       └── middleware.rs     # Axum auth extractor
│   │
│   ├── finima-ingest/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── detect.rs         # File type detection (extension + magic bytes)
│   │       ├── ofx.rs            # OFX/QFX/QBO parser
│   │       ├── qif.rs            # QIF parser
│   │       ├── csv_parser.rs     # CSV/TSV with column mapping
│   │       ├── xlsx.rs           # XLS/XLSX parser (calamine crate)
│   │       ├── dedup.rs          # Duplicate detection (SHA-256 hash)
│   │       └── preview.rs        # Preview generation for column mapping UI
│   │
│   ├── finima-llm/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── client.rs         # LLM client trait + Ollama impl + llama.cpp impl
│   │       ├── prompts.rs        # Prompt templates (categorization, enrichment)
│   │       ├── tool_defs.rs      # Tool schemas for Gemma 4 function calling
│   │       ├── categorizer.rs    # Batch categorization orchestration
│   │       └── enricher.rs       # Merchant name normalization, metadata
│   │
│   ├── finima-analysis/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── recurring.rs      # Recurring payment detection algorithm
│   │       ├── budget.rs         # Budget vs. actual computation
│   │       ├── health_score.rs   # Financial health composite score
│   │       ├── cashflow.rs       # Income vs. expense aggregation
│   │       ├── net_worth.rs      # Balance time-series computation
│   │       └── flows.rs          # Inter-account flow detection, Sankey data, waterfall, outflow ranking
│   │
│   ├── finima-feed/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── fetcher.rs        # RSS/Atom feed parser (feed-rs crate)
│   │       ├── summarizer.rs     # LLM-powered article summarization
│   │       └── relevance.rs      # Relevance scoring based on portfolio
│   │
│   └── finima-api/
│       ├── Cargo.toml
│       └── src/
│           ├── main.rs           # Entrypoint, server startup
│           ├── router.rs         # Route tree assembly
│           ├── state.rs          # AppState (pool, LLM client, config)
│           ├── config.rs         # Layered YAML config (config-rs + serde_yaml)
│           ├── handlers/         # Route handlers grouped by domain
│           │   ├── auth.rs
│           │   ├── portfolios.rs
│           │   ├── accounts.rs
│           │   ├── transactions.rs
│           │   ├── uploads.rs
│           │   ├── recurring.rs
│           │   ├── dashboard.rs
│           │   ├── budgets.rs
│           │   ├── savings.rs
│           │   ├── feed.rs
│           │   └── flows.rs
│           ├── ws.rs             # WebSocket handler
│           └── extractors.rs     # Custom Axum extractors (AuthUser, Pagination)
│
├── frontend/
│   ├── package.json
│   ├── vite.config.ts
│   ├── tsconfig.json
│   ├── Makefile
│   ├── tailwind.config.ts
│   ├── index.html
│   └── src/
│       ├── main.tsx
│       ├── App.tsx
│       ├── routes/               # React Router page components
│       ├── components/           # Reusable UI components
│       │   ├── ui/               # Primitives (Button, Input, Card, etc.)
│       │   ├── charts/           # Recharts wrappers
│       │   ├── tables/           # TanStack Table wrappers
│       │   └── layout/           # Sidebar, Header, DashboardGrid
│       ├── hooks/                # Custom hooks (useAuth, useApi, useWebSocket)
│       ├── stores/               # Zustand stores (auth, theme, preferences)
│       ├── api/                  # API client functions
│       ├── types/                # TypeScript interfaces matching backend models
│       ├── theme/                # CSS variables, theme provider
│       └── utils/                # Formatters, validators, helpers
│
├── docker-compose.yml            # Dev profile
├── docker-compose.prod.yml       # Prod profile
├── .github/
│   └── workflows/
│       ├── ci.yml                # Lint, test, build
│       └── release.yml           # Docker image build + push
├── config/
│   ├── server.yaml               # Server host/port
│   ├── database.yaml             # Database connection
│   ├── auth.yaml                 # Authentication settings
│   ├── llm.yaml                  # LLM provider config
│   ├── storage.yaml              # S3/MinIO settings
│   ├── categories.yaml           # Category hierarchy
│   ├── services.yaml             # Resend, feed, CORS
│   ├── logging.yaml              # Logging config
│   ├── development.yaml          # Dev overrides
│   ├── test.yaml                 # Test overrides (test DB, mock endpoints)
│   └── production.yaml           # Prod overrides (log level, pool size)
└── .env.example                  # Secrets only (JWT_SECRET, RESEND_API_KEY, DB password)
```

---

## 3. Key Rust Dependencies

| Crate                            | Purpose                               | Used In                              |
| -------------------------------- | ------------------------------------- | ------------------------------------ |
| `axum` 0.8+                      | HTTP framework                        | finima-api                           |
| `sqlx` 0.9+                      | Async PostgreSQL driver + migrations  | finima-db                            |
| `serde` / `serde_json`           | Serialization                         | all crates                           |
| `jsonwebtoken`                   | JWT encoding/decoding                 | finima-auth                          |
| `reqwest`                        | HTTP client (Resend API, Ollama API)  | finima-auth, finima-llm, finima-feed |
| `sha2`                           | SHA-256 hashing (magic links, dedup)  | finima-auth, finima-ingest           |
| `base64`                         | URL-safe token encoding               | finima-auth                          |
| `rand`                           | Cryptographic random token generation | finima-auth                          |
| `calamine`                       | XLS/XLSX reader                       | finima-ingest                        |
| `csv`                            | CSV/TSV parser                        | finima-ingest                        |
| `quick-xml` or `roxmltree`       | OFX/QFX XML parser                    | finima-ingest                        |
| `tokio`                          | Async runtime                         | all crates                           |
| `tower-http`                     | CORS, compression, tracing middleware | finima-api                           |
| `tracing` / `tracing-subscriber` | Structured logging                    | all crates                           |
| `config` (config-rs)             | Layered YAML configuration loading    | finima-api                           |
| `serde_yaml`                     | YAML deserialization                  | finima-api                           |
| `chrono`                         | Date/time handling                    | finima-core                          |
| `uuid`                           | UUID generation                       | finima-core                          |
| `feed-rs`                        | RSS/Atom feed parsing                 | finima-feed                          |
| `llama-cpp-4` (optional)         | Direct llama.cpp bindings             | finima-llm                           |
| `rust_decimal`                   | Precise decimal arithmetic            | finima-core                          |

---

## 4. LLM Integration Architecture

```text
                    finima-llm
                        │
            ┌───────────┴───────────┐
            │                       │
     ┌──────┴──────┐       ┌───────┴───────┐
     │ OllamaClient│       │ LlamaCppClient│
     │             │       │  (optional)   │
     │ HTTP POST to│       │ In-process    │
     │ /api/chat   │       │ llama-cpp-4   │
     │ with tools  │       │ bindings      │
     └──────┬──────┘       └───────┬───────┘
            │                       │
            │    ┌─────────────┐   │
            └────┤ Gemma 4     ├───┘
                 │ GGUF model  │
                 │ Q4_K_M      │
                 └─────────────┘

Tool-Calling Flow (Ollama path):
1. Build messages array with system prompt + tool definitions
2. POST /api/chat { model, messages, tools, stream: false }
3. Parse response.message.tool_calls[]
4. Extract structured JSON: { category, subcategory, merchant_name, confidence }
5. Update transactions in DB
```

Gemma 4 supports native function calling with 6 special tokens (`<|tool>`, `<|/tool>`, `<|tool_call>`, `<|/tool_call>`, `<|tool_result>`, `<|/tool_result>`) as documented in the [Gemma 4 function calling guide](https://ai.google.dev/gemma/docs/capabilities/text/function-calling-gemma4). Both Ollama and llama.cpp have day-one support for Gemma 4 tool calling.

---

## 5. Authentication Flow

```text
Client                    Backend (finima-auth)               Resend API
  │                              │                                │
  │  POST /auth/magic-link       │                                │
  │  { email }                   │                                │
  │─────────────────────────────▶│                                │
  │                              │ generate 32-byte random token  │
  │                              │ hash = SHA-256(token)          │
  │                              │ store hash + email + expiry    │
  │                              │ in magic_links table           │
  │                              │                                │
  │                              │  POST /emails/send             │
  │                              │  { to, subject, html }         │
  │                              │───────────────────────────────▶│
  │                              │                                │
  │  200 OK "Check your email"   │                                │
  │◀─────────────────────────────│                                │
  │                              │                                │
  │  (User clicks link in email) │                                │
  │                              │                                │
  │  POST /auth/verify           │                                │
  │  { token, email }            │                                │
  │─────────────────────────────▶│                                │
  │                              │ hash = SHA-256(token)          │
  │                              │ lookup magic_links by hash     │
  │                              │ validate: not expired,         │
  │                              │   not used, email matches      │
  │                              │ mark as used                   │
  │                              │ find or create user            │
  │                              │ issue JWT access + refresh     │
  │                              │                                │
  │  200 { access_token,         │                                │
  │        refresh_token,        │                                │
  │        user }                │                                │
  │◀─────────────────────────────│                                │
```

---

## 6. Data Flow: File Import → Categorization

```text
User                Frontend               Backend              LLM          Database
 │                    │                      │                    │               │
 │  drop CSV file     │                      │                    │               │
 │───────────────────▶│                      │                    │               │
 │                    │  POST /uploads       │                    │               │
 │                    │  (multipart + acct)  │                    │               │
 │                    │─────────────────────▶│                    │               │
 │                    │                      │ detect file type   │               │
 │                    │                      │ parse headers      │               │
 │                    │                      │                    │               │
 │                    │  200 { preview,      │                    │               │
 │                    │    columns, upload_id}│                   │               │
 │                    │◀─────────────────────│                    │               │
 │                    │                      │                    │               │
 │  confirm mapping   │                      │                    │               │
 │───────────────────▶│                      │                    │               │
 │                    │  POST /uploads/:id/  │                    │               │
 │                    │  confirm { mapping } │                    │               │
 │                    │─────────────────────▶│                    │               │
 │                    │                      │ parse all rows     │               │
 │                    │                      │ compute dedup hash │               │
 │                    │                      │ INSERT transactions│──────────────▶│
 │                    │                      │                    │               │
 │                    │                      │ batch 20 txns      │               │
 │                    │                      │────────────────────▶               │
 │                    │                      │                    │ categorize    │
 │                    │                      │                    │ (tool call)   │
 │                    │                      │◀───────────────────│               │
 │                    │                      │ UPDATE transactions│──────────────▶│
 │                    │  WS: progress event  │                    │               │
 │                    │◀─────────────────────│                    │               │
 │  see progress bar  │                      │                    │               │
 │◀───────────────────│                      │                    │               │
 │                    │  WS: complete event   │                   │               │
 │                    │◀─────────────────────│                    │               │
 │  see dashboard     │                      │                    │               │
 │◀───────────────────│                      │                    │               │
```

---

## 7. Frontend State Architecture

```text
Zustand Stores
├── authStore        → { user, accessToken, refreshToken, isAuthenticated, login(), logout() }
├── themeStore       → { mode, accentColor, setMode(), setAccent() }
├── prefsStore       → { currency, dateFormat, fiscalMonth, dashboardLayout, updatePref() }
├── portfolioStore   → { portfolios, activePortfolioId, accounts, selectPortfolio() }
└── wsStore          → { socket, lastMessage, connect(), disconnect() }

React Router v7 Routes
├── /                → LandingPage (unauthenticated) | Dashboard (authenticated)
├── /auth/signin     → SignInPage
├── /auth/verify     → VerifyPage (handles magic link callback)
├── /onboarding      → OnboardingWizard
├── /dashboard       → Dashboard
├── /accounts        → AccountsList
├── /accounts/:id    → AccountDetail
├── /transactions    → TransactionsPage (aggregate)
├── /recurring       → RecurringPage
├── /flows           → MoneyFlowPage (Sankey, Balance Impact, Flow Groups)
├── /budget          → BudgetPage
├── /goals           → SavingsGoalsPage
├── /news            → NewsFeedPage
└── /settings        → SettingsPage
```

---

## 8. Security Considerations

- **Magic link tokens:** 32 bytes of `rand::rngs::OsRng`, base64url-encoded, SHA-256 hashed before storage. Never stored in plaintext.
- **JWT:** HMAC-SHA256 signed. Short-lived access tokens (15 min) with longer refresh tokens (7 days, single-use rotation).
- **CORS:** Strict origin whitelist (`http://localhost:5173` in dev, configured domain in prod).
- **Rate limiting:** tower-http rate limit on `/auth/magic-link` (5 requests per email per hour).
- **Input validation:** All user inputs validated with Rust types (UUIDs, enums, decimal bounds). File uploads size-limited (50MB max).
- **SQL injection:** Prevented by SQLx parameterized queries (compile-time checked).
- **Data at rest:** PostgreSQL with `pgcrypto` for sensitive fields. Optional full-disk encryption at OS level.
- **File upload safety:** File type validated by magic bytes, not just extension. No server-side execution of uploaded content.

---

## 9. References

- Axum web framework: [docs.rs/axum](https://docs.rs/axum)
- SQLx async Postgres: [docs.rs/sqlx](https://docs.rs/sqlx)
- llama-cpp-4 crate with tool-calling support: [github.com/utilityai/llama-cpp-rs](https://github.com/utilityai/llama-cpp-rs)
- Gemma 4 model card: [ai.google.dev/gemma/docs/core/model_card_4](https://ai.google.dev/gemma/docs/core/model_card_4)
- Gemma 4 function calling: [ai.google.dev/gemma/docs/capabilities/text/function-calling-gemma4](https://ai.google.dev/gemma/docs/capabilities/text/function-calling-gemma4)
- Ollama Gemma 4 support: [lmstudio.ai/models/gemma-4](https://lmstudio.ai/models/gemma-4)
- Resend API: [resend.com](https://resend.com)
- calamine (XLS/XLSX): [docs.rs/calamine](https://docs.rs/calamine)
- feed-rs (RSS/Atom): [docs.rs/feed-rs](https://docs.rs/feed-rs)
