# ADR-014: Net Worth as Primary Dashboard Organizing Frame

## Status

Proposed

## Date

2026-04-15

## Context

Personal finance applications organize their primary dashboard around one of three
philosophical frames:

| Frame                       | Representative apps                       | Core question answered            |
| --------------------------- | ----------------------------------------- | --------------------------------- |
| **Net worth first**         | Empower, NerdWallet, Monarch, Wealthfront | "How much am I worth today?"      |
| **Spending / budget first** | Copilot, YNAB, PocketGuard, Simplifi      | "How much can I spend right now?" |
| **Goals first**             | Betterment, Wealthfront                   | "Am I on track to retire?"        |

Research across 12 applications (April 2026) found:

> "Net worth tracking (assets minus liabilities) with a historical chart appears in all
> but the most niche apps (Honeydue, Buddy). The convention is: live figure updated from
> linked accounts, shown with a line/area chart."
> — Dashboard UX Research Report, April 2026

Empower places net worth as the hero metric at the top of the page with a historical
progression chart. Monarch places it as the top widget in the default layout.
NerdWallet's 2025 redesign moved net worth to the headline position after user research.

Finima's current user base tracks a household portfolio (one joint checking account as
of April 2026). The data model is portfolio-centric: one portfolio can hold multiple
accounts across multiple asset types. As users add investment, savings, and loan
accounts the net worth frame scales naturally. A spending-first frame would require
budget configuration before it provides value, creating a cold-start friction problem.

## Decision

**Net worth with a 90-day delta is the primary organizing metric on the Finima dashboard.**

The top of the dashboard shows, in order:

1. **Net Worth** (large) — live balance, see ADR-013
2. **30-day change** — dollar and percent delta from 30 days prior
3. **Net worth sparkline** — 90-day line chart, no axes

Secondary summary cards (the existing row below net worth) carry:

- Total Assets, Total Liabilities, Account Count

Below the summary row, widgets are ordered by the following default priority:

1. Net Worth chart (12-month area chart)
2. Financial Health score
3. Cash Flow (6-month income vs expenses bar chart)
4. Spending by Category (current month)
5. Upcoming bills/recurring
6. Budget vs Actual

This order aligns with the Empower and Monarch default layouts, which research found to
be the most frequently copied hierarchy in user-created comparisons.

## Consequences

### Positive

- The dashboard provides immediate value to a new user with one linked account and no
  budgets configured — net worth is computable from day one.
- Scales to multi-account, multi-institution portfolios without requiring user setup.
- Consistent with the market convention that users migrating from Empower, NerdWallet,
  or Monarch expect.

### Tradeoff

- Users coming from Copilot or YNAB expect spending/budget data above the fold. These
  users are served by the widget customisation capability (see ADR-015) — they can
  promote the spending widget to the top.
- A safe-to-spend figure (see ADR-016) will also appear in the summary row once
  implemented, which partially addresses spending-first expectations without demoting
  net worth.
