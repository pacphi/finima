# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-04-23

### Bug Fixes

- **flows:** Donut breakdown larger + no longer clipped at top
- **flows:** Outflow ranking mirrors Sankey + humanized Type column
- **dashboard:** Drill into "Other" slice to show rolled-up categories
- **dashboard:** Show category labels in Budget vs Actual tile

### Dependencies

- **rust:** Bump openssl from 0.10.77 to 0.10.78 (#37)

### Documentation

- ADR-018, ADR-008 Amendment 2, DDD-003/005 updates, ADR index
- **ruvector:** Phase 0 spike findings (0, 0b, 0c, 0d) (#22)
- Add user interface overview

### Features

- **core:** TransactionDirection + AccountRole + migration 023
- **core:** SignNormalizer service for institution-aware sign resolution
- **core:** SignAutodetector for institution-free convention inference
- **api:** YAML sign_conventions + sankey config sections
- **ingest+api:** Normalize TransactionDirection at import time
- **api:** Finima-normalize-directions CLI for direction backfill
- **flows:** Emit spender-role virtual nodes for primary direct spending
- **accounts:** Per-account sign-convention override (UI + API)
- **flows:** Category-row View links to filtered Transactions page
- **recurring:** Configurable variable threshold + median classifier + UI filter
- Plaid-aligned recurring detection, canonical amounts, account purge, outcome-prefix categorization
- Tier 2 semantic categorization + SONA-enhanced flow detection (#30)
- **dashboard:** Redesign + recurring history, balance-history API (#36)
- **sample + portfolios:** Demo fixture + per-portfolio scoping across stack
- **portfolios:** Cascade delete + shared confirm-delete dialog
- **release:** Tag-driven releases with auto-changelog and version pill (#38)

### Refactors

- **flows:** Query direction=outflow instead of sign heuristic
- **flows:** Simplify InteractiveSankey + dashed spender-role nodes
