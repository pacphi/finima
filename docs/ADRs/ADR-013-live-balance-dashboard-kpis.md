# ADR-013: Live Account Balance for All Dashboard Summary KPIs

## Status

Accepted

## Date

2026-04-15

## Context

The dashboard `GET /api/dashboard/summary` handler previously computed net worth,
total assets, and total liabilities by calling `compute_net_worth_series(today, today)`.
That function normalises its start date to the **first of the current month** and filters
transactions with `date < first_of_month`. On 15 April 2026, this produced a net worth
of **$7,696** while the Accounts page (which calls `compute_balance` — a direct SQL sum
of `opening_balance + ALL transactions`) correctly showed **$239,742.36** for the same
account.

The same bug existed in `GET /api/dashboard/health-score`.

The root cause was a mismatch between two balance-computation strategies:

| Strategy                   | Used by                            | Semantics                                            |
| -------------------------- | ---------------------------------- | ---------------------------------------------------- |
| `compute_net_worth_series` | Dashboard (pre-fix)                | `opening_balance + txns WHERE date < first_of_month` |
| `compute_balance` (SQL)    | Accounts page, liquid-savings loop | `opening_balance + SUM(ALL txns)`                    |

Research into comparable applications confirms the expected convention:

> "Every app that shows net worth displays it as a continuously updated figure drawn from
> linked accounts, not a periodic snapshot."
> — Dashboard UX Research Report, April 2026

Users encountering a dashboard net worth that is $232K lower than the figure on the
Accounts page lose trust in the application immediately.

## Decision

**All summary KPIs on the dashboard that represent "current state" are computed using
`compute_balance` per account (the SQL-backed real-time sum), not via
`compute_net_worth_series`.**

Specifically, `get_summary` and `get_health_score` in
`crates/finima-api/src/handlers/dashboard.rs` now iterate over non-archived accounts
and call `account_repo.compute_balance(acct.id)` for each, accumulating:

- `total_assets` — sum of `compute_balance` for non-liability accounts
- `total_liabilities` — sum of `compute_balance.abs()` for liability accounts
  (CreditCard, LoanMortgage, LoanAuto, LoanStudent, LoanPersonal)
- `liquid_savings` — sum of `compute_balance` for Checking, Savings, Cash
- `net_worth = total_assets − total_liabilities`

`compute_net_worth_series` is **retained** for the `GET /api/dashboard/net-worth`
time-series endpoint, where the monthly snapshot semantics are correct and intentional
(each point represents the account balance as of the first of a given month, used for
the chart).

## Consequences

### Positive

- Net worth on the dashboard always matches the Accounts page.
- Financial health score inputs (savings rate, debt ratio, emergency months) are derived
  from today's actual balances, not a 15-day-stale snapshot.
- Eliminates a category of user confusion that is fatal to trust in a personal finance
  application.

### Tradeoff

- The summary endpoint now issues one `SELECT` per non-archived account instead of one
  aggregated computation. For a user with 10 accounts this is 10 queries vs 1 analysis
  pass. Given typical portfolio sizes (2–15 accounts) this is acceptable. If profiling
  identifies this as a bottleneck, the per-account queries can be replaced by a single
  joined SQL aggregate.

### Out of scope

- The net worth chart time series (`/api/dashboard/net-worth`) intentionally retains
  `compute_net_worth_series` month-boundary semantics — each chart point is a
  first-of-month snapshot, which is the correct representation for a historical trend.
