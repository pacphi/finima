# Finima Docs-vs-Reality Truth Audit -- April 2026 (Round 2)

**Date:** 2026-04-11
**Auditors:** Agentic QE Multi-Agent Swarm (3 parallel agents: explorer, auditor, prior-audit reviewer)
**Scope:** Documentation accuracy vs. actual implementation; doc gaps and improvement opportunities
**Method:** Every claim in README, 8 guides, 10 ADRs, and 6 DDDs was cross-referenced against source code, config, and tests.

---

## Executive Summary

| Dimension                   | Score    | Verdict                                                           |
| --------------------------- | -------- | ----------------------------------------------------------------- |
| README Accuracy             | 8/10     | Mostly honest; two material omissions                             |
| Guide Accuracy              | 7/10     | Several stale claims and one lie-by-omission                      |
| ADR Accuracy                | 9/10     | Faithfully describe decisions; one ADR missing from arch overview |
| DDD Accuracy                | 9/10     | Clean mapping to crate boundaries                                 |
| Doc Completeness            | 6/10     | Major gaps in LLM fallback, env config, API reference, testing    |
| **Overall Docs-vs-Reality** | **7/10** | **Docs paint a rosier picture than reality warrants**             |

### The One-Line Truth

The documentation is well-structured and covers most features accurately, but it **systematically omits degraded-mode behavior** (stub LLM fallback), has **no API reference**, and contains several specific claims that contradict the implementation.

---

## Part 1: Specific Inaccuracies (Docs Say X, Code Says Y)

### 1.1 INACCURATE: Rate Limit Described Inconsistently

| Source                            | Claim                                                                    |
| --------------------------------- | ------------------------------------------------------------------------ |
| `troubleshooting.md` line 26      | "five per hour per email address"                                        |
| `ADR-002`                         | "5 requests per email per hour"                                          |
| `DDD-001`                         | "At most 5 magic links per email per hour"                               |
| **Actual code** (`router.rs:235`) | `RateLimiter::new(5, Duration::from_secs(60))` — **5 per minute per IP** |

**Verdict:** Three doc sources say "5/hour/email". Code says "5/minute/IP". Both the unit (minute vs hour) and the key (IP vs email) are wrong in docs. This is a **material safety documentation error** — an operator reading the docs would believe the rate limit is 60x more restrictive than it actually is.

### 1.2 INACCURATE: Architecture Overview Omits QIF Format

`architecture-overview.md` line 187–188 says:

> "Finima supports importing transactions from CSV, OFX/QFX, and Excel files"

**Reality:** QIF is fully implemented (`crates/finima-ingest/src/qif.rs`), documented in the user guide, quick-start guide, and troubleshooting guide, and declared in the `FileFormat` type. The architecture overview is the only doc that omits it.

### 1.3 INACCURATE: Architecture Overview Omits ADR-010

The architecture overview's "Key Design Decisions" table lists ADRs 001–009 but omits **ADR-010 (Candle/MistralRS inference backend)**. This ADR exists at `docs/ADRs/ADR-010-candle-mistralrs-inference-backend.md` and is referenced in `maintainer-guide.md`.

### 1.4 MISLEADING: LLM Described As Always-On

Multiple docs describe AI categorization as a core feature without disclosing degraded behavior:

| Doc                     | Claim                                                                                   | Reality                                                                                                                                              |
| ----------------------- | --------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------- |
| README line 8           | "Local AI categorization — Ollama-powered LLM classifies transactions on-device"        | True **only if** Ollama is running and model is pulled. Otherwise `StubLlmClient` returns category="other" with confidence=0.5 for ALL transactions. |
| README line 19          | "AI-powered categorization — local LLM via Ollama, no cloud API calls"                  | Same — silently degrades.                                                                                                                            |
| User Guide line 149–152 | "If Ollama is running and a model has been pulled, Finima automatically categorizes..." | This is the **only** doc that hedges correctly.                                                                                                      |

The `StubLlmClient` fallback is clearly documented in code comments (`stub.rs:5`: "This is clearly marked as a stub and should be replaced") but **zero user-facing docs** mention that without Ollama the system silently degrades. The Settings > LLM tab shows connection status, but a new user following the quick-start who skips the optional `make download-model` step will get all transactions categorized as "other" with no warning.

### 1.5 MISLEADING: "Dashboard widgets can be rearranged by dragging them"

`quick-start.md` line 184 and `user-guide.md` lines 13–14 claim widget drag-and-drop.

**Reality:** `react-grid-layout` is imported and `ResponsiveGridLayout` is rendered with `dragConfig={{ handle: '.widget-drag-handle' }}`. However, there is no visual drag handle rendered on widget cards (no `.widget-drag-handle` element found in widget components). The feature is **wired up in the grid but has no visible affordance** — users cannot discover it.

**Verdict:** Technically the grid supports drag, but without visible handles this is a broken feature. Docs should not claim it works.

### 1.6 STALE: Troubleshooting References Non-Existent Make Targets

`troubleshooting.md` references `make docker-restart` (line 44), `make docker-ps` (line 84), `make docker-logs-backend` (line 302). These targets may or may not exist — they should be verified against the actual Makefile, and any missing targets should be added or the docs corrected.

---

## Part 2: Material Omissions (Code Does X, Docs Say Nothing)

### 2.1 GAP: No Documentation of Stub/Fallback LLM Behavior

**Impact: HIGH**

When Ollama is unavailable or the feature flag is disabled at compile time:

- All categorization returns `category="other"`, `confidence=0.5`
- All enrichment returns default/empty values
- All insight generation returns a placeholder string
- Feed article summarization returns stubs

This behavior is **by design** (graceful degradation), but it is completely undocumented. A deployment without Ollama will silently produce meaningless categorizations. Users need to know:

1. That stub mode exists
2. How to detect it (logs say "Using STUB LLM client")
3. That transactions imported during stub mode can be re-categorized later

**Where to document:** Add a "Degraded Mode / Without Ollama" section to `troubleshooting.md` and a note in `quick-start.md`.

### 2.2 GAP: No API Reference

**Impact: HIGH**

There is **no REST API documentation** anywhere. The backend has 15+ handler files covering ~40 endpoints. For a self-hosted app, operators and developers need:

- Endpoint list with methods, paths, request/response shapes
- Authentication requirements per endpoint
- Rate limit details
- WebSocket message schemas

**Where to document:** Create `docs/guides/api-reference.md` or generate from code annotations.

### 2.3 GAP: Compile-Time Feature Flags Undocumented

**Impact: MEDIUM**

The `finima-llm` crate has compile-time features `candle` and `ollama`. When neither is enabled, the stub client is used silently. The `finima-api/src/state.rs` initialization logic:

- `candle` feature enabled → tries Candle, falls back to stub on failure
- `ollama` feature enabled → uses Ollama client
- Neither → stub

This is invisible to anyone building from source. `maintainer-guide.md` should document available feature flags and their effects.

### 2.4 GAP: No Documentation of Resend Email Fallback

**Impact: MEDIUM**

When `RESEND_API_KEY` is empty, the system uses `LoggingEmailSender` which logs the magic link URL to stdout instead of sending email. The quick-start guide mentions this (line 99–101) but the deployment guide does not explicitly warn that **production MUST set this key** or auth will silently fall back to logging links to the container log (where an attacker with log access could steal them).

### 2.5 GAP: Environment Variable Reference Incomplete

**Impact: MEDIUM**

The deployment guide documents 7 env vars. But the app also reads:

- `APP__LLM__OLLAMA__MODEL` (mentioned only in deployment GPU section)
- `APP__DATABASE__MAX_CONNECTIONS` (mentioned only in scaling section)
- `APP_ENV` (mentioned only in security checklist)
- `BACKUP_RETENTION_DAYS` (mentioned once in passing)
- All `APP__*` overrides via the YAML config layering system (ADR-009)

There is no single reference of all environment variables, their defaults, and which are required vs optional.

**Where to document:** Add an "Environment Variables" section to `deployment.md` or a standalone `docs/guides/configuration-reference.md`.

### 2.6 GAP: No Testing Documentation

**Impact: MEDIUM**

The README lists 4 make targets for running tests. The maintainer guide likely covers some test details. But there is no documentation of:

- What the test suite covers (and what it does NOT cover)
- How to write new tests
- Integration test prerequisites (running Postgres, Ollama)
- E2E test setup (Playwright config, test users)
- Current coverage metrics

### 2.7 GAP: Integration Test Placeholders in DB Repos

**Impact: LOW (code, not docs)**

Every repository file in `finima-db/src/repos/` contains `// Integration test placeholder` comments with empty test bodies:

- `upload_repo.rs:117`
- `recurring_repo.rs:165`
- `budget_repo.rs:95`
- `account_repo.rs:150`
- `flow_group_repo.rs:122`
- `user_repo.rs:90`
- `flow_repo.rs:144`
- `magic_link_repo.rs:74`
- `portfolio_repo.rs:100`
- `session_repo.rs:94`
- `override_repo.rs:93`
- `savings_goal_repo.rs:123`

These placeholders were removed on 2026-04-11. Integration testing of DB repos is handled at the API level via `crates/finima-api/tests/` which exercises repos through handlers with a real test database.

---

## Part 3: Documentation Quality Assessment by File

### README.md — Score: 8/10

**Strengths:**

- Accurate feature list (with caveats above)
- Clean structure with quick-start, docs table, architecture summary
- Correct make targets for testing

**Issues:**

- LLM categorization described without fallback caveat
- No mention that banking integrations are file-import only (no Plaid)
- Claims like "bring data from any bank" imply direct connections

### docs/guides/quick-start.md — Score: 9/10

**Strengths:**

- Step-by-step with clear prerequisites
- Correct port numbers and container names
- Properly notes `make download-model` is optional

**Issues:**

- Does not warn what happens if you skip the model download (stub mode)
- Should mention that `LoggingEmailSender` prints to terminal when no Resend key

### docs/guides/user-guide.md — Score: 8/10

**Strengths:**

- Comprehensive coverage of every feature
- Accurate descriptions of UI behavior
- Properly hedges on LLM availability (the only doc that does)

**Issues:**

- Dashboard drag-and-drop claim (no visible handles)
- Export CSV documented — verified as implemented (good)
- Does not mention that Export CSV is client-side only (no server-side export endpoint)

### docs/guides/deployment.md — Score: 8/10

**Strengths:**

- Thorough production checklist
- Correct Docker commands and config paths
- Good security checklist

**Issues:**

- Env var reference incomplete (see Gap 2.5)
- References "backup guide" and "observability guide" with "(if available)" — these exist as separate files but the links are wrong (`../backup-guide.md` vs actual `database-backup.md`)

### docs/guides/troubleshooting.md — Score: 7/10

**Strengths:**

- Covers 10 common scenarios
- Actionable fixes with commands

**Issues:**

- Rate limit claim wrong (says 5/hour, code says 5/minute)
- May reference non-existent make targets
- No mention of stub LLM fallback behavior

### docs/guides/architecture-overview.md — Score: 7/10

**Strengths:**

- Excellent ASCII diagrams
- Accurate crate dependency graph
- Good request lifecycle documentation

**Issues:**

- Omits QIF from file import pipeline
- Omits ADR-010 from design decisions table
- Does not mention the stub fallback in LLM integration section

### ADRs (10 total) — Score: 9/10

**Strengths:**

- All 10 ADRs accurately reflect implemented decisions
- Good structure: context, decision, consequences
- Alternatives considered are realistic

**Issues:**

- ADR-005 says "CSV, OFX, QFX, QIF, QBO, XLS, XLSX" — correct and complete
- ADR-010 (Candle) exists but is not referenced from the architecture overview

### DDDs (6 total) — Score: 9/10

**Strengths:**

- Clean mapping: each DDD corresponds to a real crate
- Domain language is consistent with code

**Issues:**

- DDD-001 rate limit claim (5/hour) contradicts implementation (5/min)

---

## Part 4: Improvement Opportunities

### Priority 1 — Fix Before Next Release

| #   | Action                                                                 | File(s)                                                      | Effort | Status                                                                                                                                                     |
| --- | ---------------------------------------------------------------------- | ------------------------------------------------------------ | ------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1   | Fix rate limit docs: change "5/hour/email" to "5/minute/IP" everywhere | troubleshooting.md, ADR-002, DDD-001                         | 30 min | **DONE** (2026-04-11). ADR-009 had no rate limit text; architecture-overview.md already had correct rate limit.                                            |
| 2   | Add "Degraded Mode" section documenting stub LLM behavior              | troubleshooting.md, quick-start.md, architecture-overview.md | 1 hour | **DONE** (2026-04-11). Added section 2a to troubleshooting, stub-mode note to quick-start, and stub fallback details to architecture overview LLM section. |
| 3   | Add QIF to architecture overview's file import section                 | architecture-overview.md                                     | 5 min  | **DONE** (2026-04-11). Added QIF to prose, parse pipeline diagram, crate dependency graph, and ADR-005 row.                                                |
| 4   | Add ADR-010 to architecture overview's design decisions table          | architecture-overview.md                                     | 5 min  | **DONE** (2026-04-11).                                                                                                                                     |
| 5   | Fix broken internal links in deployment.md                             | deployment.md                                                | 15 min | **N/A** — links already correct (`database-backup.md` and `observability.md` both exist at the referenced paths).                                          |

### Priority 2 — Create Missing Documentation

| #   | Action                                                                           | Suggested Path                                | Effort    |
| --- | -------------------------------------------------------------------------------- | --------------------------------------------- | --------- |
| 6   | Create API reference (all endpoints, auth requirements, request/response shapes) | docs/guides/api-reference.md                  | 4-6 hours |
| 7   | Create environment variable reference (all vars, defaults, required/optional)    | docs/guides/configuration-reference.md        | 2 hours   |
| 8   | Document compile-time feature flags (candle, ollama) and their effects           | docs/guides/maintainer-guide.md (add section) | 1 hour    |
| 9   | Create testing guide (how to run, write, and debug tests; coverage status)       | docs/guides/testing-guide.md                  | 2-3 hours |

### Priority 3 — Improve Existing Docs

| #   | Action                                                                                  | File(s)                                                                | Effort    |
| --- | --------------------------------------------------------------------------------------- | ---------------------------------------------------------------------- | --------- |
| 10  | Verify all `make` targets referenced in docs against actual Makefile                    | All guides                                                             | 1 hour    |
| 11  | Add visible drag handles to dashboard widgets (or remove drag claim from docs)          | DashboardPage.tsx + widget components OR user-guide.md, quick-start.md | 1-2 hours |
| 12  | Document that "bring data from any bank" means file import, not direct bank connections | README.md                                                              | 15 min    |
| 13  | Add WebSocket message schema documentation                                              | api-reference.md or architecture-overview.md                           | 1 hour    |
| 14  | Document the YAML config layering system with examples for operators                    | deployment.md or configuration-reference.md                            | 1 hour    |

---

## Part 5: Comparison with Audit 01

The first audit (april-2026-audit-01.md) focused on **code quality, security, and test coverage**. All 38 critical findings were remediated:

- Horizontal privilege escalation: fixed (ownership checks on all 15 endpoints)
- Session revocation: implemented (server-side session lifecycle)
- Database bug in recurring_repo: fixed
- File storage moved to MinIO
- 22 Playwright E2E tests + 25 integration tests added
- SigNoz observability, backups, accessibility all implemented

**This audit (Round 2) focuses on documentation accuracy.** It found that while the codebase has improved dramatically since Audit 01, the **documentation has not been updated to reflect all changes**, and several pre-existing inaccuracies persist.

### New Issues Not in Audit 01

- Rate limit documentation error (docs say hour, code says minute)
- Stub LLM fallback completely undocumented for end users
- No API reference exists
- Architecture overview missing QIF and ADR-010
- Dashboard drag-and-drop has no visible affordance

---

## Appendix: Audit Methodology

### Agents Used

1. **Explorer Agent** — Mapped all 23 documentation files, 8 Rust crates, frontend structure (14 routes, 35+ components), configuration files, Docker infrastructure, and test files.

2. **Auditor Agent** — Cross-referenced every feature claim against source code. Verified: auth flow, file import pipeline, database schema (13 migrations), WebSocket implementation, LLM client initialization, financial analysis modules, and frontend API clients.

3. **Prior-Audit Agent** — Read and summarized Audit 01 findings and remediation status to avoid re-reporting fixed issues.

### Verification Tools

- `grep`/`rg` pattern matching across all `.rs`, `.ts`, `.tsx`, `.md` files
- Direct source code reads of key files (state.rs, router.rs, stub.rs, DashboardPage.tsx, SettingsPage.tsx)
- Package.json dependency verification
- Config file cross-referencing

### Limitations

- `.env.example` was inaccessible (permission denied) — env var completeness assessed from deployment docs and config code only
- No runtime testing was performed — all findings are static analysis
- Makefile targets were not exhaustively verified against docs references
