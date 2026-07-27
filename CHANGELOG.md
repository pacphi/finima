# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0] - 2026-07-27

### Bug Fixes

- **lychee:** Remove plans exclusion; fix 6 relative links in plan to resolve from plans/ directory
- **lychee:** Resolve 3 external link failures — exclude defunct subaio/intuit URLs, remove dead oreateai link
- **ci:** Pin lychee to v0.24.2; fix MD040 unlabeled code fences and plan relative links
- **lychee:** Remove version pin and include-fragments from TOML; v0.24 archive layout breaks lychee-action@v2 install

### Dependencies

- Apply all 12 dependabot upgrades (#39-#50)
- **rust:** Bump tokenizers 0.22 → 0.23; add RUSTSEC-2026-0002 audit ignore
- **rust:** Update aws-sdk-s3 1.119→1.132, lru 0.12.5→0.16.4 (fixes RUSTSEC-2026-0002)
- **ts:** Update postcss 8.5.9→8.5.14 (fixes CVE-2026-41305 XSS advisory)
- Apply 10 Dependabot bumps from open PRs (#51–#60)

### Documentation

- Add plain-language Docker and Git install preamble to quick start
- Add missing steps 4 and 5 to quick start (wait for ready, verify running)
- Rewrite UI overview captions in plain language for non-technical users
- Rewrite LLM settings section in plain language for end users
- Add bank statement download guidance to importing transactions section
- Add 'Your First Week with Finima' getting-started guide for non-technical users
- Add plain-language glossary for financial and app terms
- Add bank statement download guidance and troubleshooting callouts to quick start
- Add non-technical documentation paths to README
- Tech marketing overhaul for README

### Features

- **tier2:** Observability gauges, E2E persistence test, staging rollout enablement (#100)

## [0.1.0] - 2026-04-23

### Bug Fixes

- **flows:** Donut breakdown larger + no longer clipped at top
- **flows:** Outflow ranking mirrors Sankey + humanized Type column
- **dashboard:** Drill into "Other" slice to show rolled-up categories
- **dashboard:** Show category labels in Budget vs Actual tile
- **frontend:** Pin Docker base to node:24-alpine (LTS)

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
