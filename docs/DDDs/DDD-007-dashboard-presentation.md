# DDD-007: Dashboard Presentation Bounded Context

**Date:** 2026-04-15  
**Layer:** `finima-api` (dashboard handlers) + `frontend/src/routes/DashboardPage.tsx`  
**Upstream contexts:** Financial Analysis (DDD-005), Portfolio Management (DDD-002), Intelligence (DDD-004)

---

## 1. Purpose

Translates computed financial analysis data into the user-facing dashboard
representation. This context owns the _assembly_ of per-widget payloads, the
_derived summary metrics_ that span multiple analysis outputs (e.g., safe-to-spend),
and the _display conventions_ (time-period framing, metric formatting, alert ordering).

It does **not** compute raw financial data — that is the responsibility of DDD-005
(Financial Analysis). It does **not** manage account data — that is DDD-002.

---

## 2. Ubiquitous Language

| Term                          | Definition                                                                                                                                                                                                                                   |
| ----------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Dashboard Summary**         | The top-level payload returned by `GET /api/dashboard/summary`, containing all headline KPIs for a single portfolio.                                                                                                                         |
| **Widget**                    | An independently fetchable, independently renderable UI panel on the dashboard. Each widget has its own API endpoint and its own loading/error state.                                                                                        |
| **Net Worth Delta**           | The change in net worth between today and a reference date (30 days ago by default), expressed as both a dollar amount and a percentage.                                                                                                     |
| **Safe to Spend**             | A computed value: (expected remaining income for the month) − (remaining scheduled outflows for the month) − (savings goal contributions). Represents discretionary money available without derailing the plan.                              |
| **Monthly Pace**              | The ratio of current MTD spending to the expected spend at the current point in the month, based on prior 3-month average daily spend. Values > 1.05 are "running hot"; < 0.95 are "running low".                                            |
| **Category Health Indicator** | Per-category display combining: MTD spend, projected month-end at current pace, monthly average (trailing 3 months), and a pace label.                                                                                                       |
| **Upcoming Obligation**       | A confirmed or detected recurring payment expected within the next 7 days. Includes payee name, expected amount, and days until due.                                                                                                         |
| **Time Window**               | The date range applied to period-relative widgets. One of: `mtd` (1st of month through today), `rolling_30` (today − 30 days through today), or `last_month` (complete prior calendar month).                                                |
| **Alert**                     | A user-facing notification surfaced in a consolidated Alerts panel, priority-ordered: (1) overdue obligations, (2) overspent categories, (3) unusual spending, (4) goal milestones, (5) informational.                                       |
| **Spending Trend**            | A directional indicator (Increasing, Stable, Decreasing) comparing last month's total expense to the 3-month trailing average. Not a percentage — a categorical label.                                                                       |
| **Health Score**              | A composite 0–100 metric (computed by DDD-005 HealthScorer). Displayed in this context with a gauge, a label (Poor/Fair/Good/Great/Excellent), and a four-metric breakdown (savings rate %, debt ratio %, emergency months, spending trend). |
| **Sync Freshness**            | The elapsed time since the portfolio's accounts were last successfully synced from the bank. Displayed as "updated X min ago" adjacent to live KPIs.                                                                                         |

---

## 3. Aggregates

### DashboardSummary (Read Model — not persisted)

```text
DashboardSummary
  net_worth:           Decimal   // live: total_assets − total_liabilities
  net_worth_delta_30d: Decimal   // net_worth − net_worth_30_days_ago
  net_worth_delta_pct: f64       // delta / net_worth_30_days_ago * 100
  total_assets:        Decimal   // sum of compute_balance for non-liability accounts
  total_liabilities:   Decimal   // sum of compute_balance.abs() for liability accounts
  account_count:       usize     // non-archived accounts
  monthly_income:      Decimal   // current month income from cashflow
  monthly_expenses:    Decimal   // current month expenses from cashflow
  safe_to_spend:       Decimal   // (planned) see SafeToSpend service
  savings_rate:        f64       // 0.0–1.0 from HealthScorer
  health_score:        u8        // 0–100 composite
  upcoming_bills_count: i64      // confirmed recurring groups due in 7 days
```

**Invariants:**

- `net_worth = total_assets − total_liabilities` (derived, not stored).
- All monetary values use today's `compute_balance` (real-time SQL), not monthly snapshots (see ADR-013).
- `savings_rate` is always in [0.0, 1.0]; the UI multiplies by 100 before display.

---

### SafeToSpend (Value Object — computed on demand)

```text
SafeToSpend
  expected_remaining_income: Decimal    // planned income not yet received this month
  remaining_obligations:     Decimal    // sum of upcoming bills before month-end
  goal_contributions:        Decimal    // scheduled savings goal transfers this month
  discretionary:             Decimal    // = expected_income − obligations − goals
  as_of:                    DateTime   // computation timestamp
```

**Computation rule:**

```text
discretionary = (monthly_income_estimate − income_received_mtd)
              + remaining_confirmed_recurring_income
              − remaining_confirmed_recurring_expenses
              − savings_goal_monthly_contributions
```

Where `monthly_income_estimate` is the trailing 3-month average monthly income.

**Invariants:**

- `discretionary` can be negative (overspent plan); display in red.
- Does not include irregular/non-recurring outflows (only confirmed recurring groups).

---

### CategoryHealthIndicator (Value Object — per category)

```text
CategoryHealthIndicator
  category:         String
  mtd_spent:        Decimal
  monthly_avg:      Decimal     // trailing 3-month average
  projected_eom:    Decimal     // mtd_spent / (days_elapsed / days_in_month)
  pace_ratio:       f64         // projected_eom / monthly_avg
  pace_label:       PaceLabel   // { OnTrack | RunningHot | RunningLow }
```

**Pace thresholds:**

- `RunningHot` if `pace_ratio > 1.10`
- `OnTrack` if `0.90 ≤ pace_ratio ≤ 1.10`
- `RunningLow` if `pace_ratio < 0.90`

Inspired by Simplifi's Watchlist four-horizon view (MTD, monthly avg, YTD, projected
month-end) as cited in the Dashboard UX Research Report, April 2026.

---

### UpcomingObligation (Entity — from confirmed recurring groups)

```text
UpcomingObligation
  recurring_group_id: UUID
  merchant_name:      String
  expected_amount:    Decimal    // avg_amount from recurring group
  days_until_due:     i32        // next_expected_date − today
  urgency:            Urgency    // { Overdue | Today | Soon | Upcoming }
```

**Urgency mapping:**

- `Overdue`: `days_until_due < 0`
- `Today`: `days_until_due = 0`
- `Soon`: `1 ≤ days_until_due ≤ 2`
- `Upcoming`: `3 ≤ days_until_due ≤ 7`

---

### NetWorthDelta (Value Object)

```text
NetWorthDelta
  current:      Decimal
  prior:        Decimal     // net worth as of reference date
  delta_amount: Decimal     // current − prior
  delta_pct:    f64         // delta / prior.abs() * 100
  reference_period: DeltaPeriod  // { Days30 | Days90 | YTD }
```

**Invariants:**

- If `prior = 0`, `delta_pct` is undefined and displayed as "—".
- `delta_amount` positive → rendered green; negative → rendered red.

---

## 4. Domain Services

### DashboardAssembler

Primary read-model builder for the summary endpoint.

- `assemble_summary(portfolio_id) -> DashboardSummary`
  1. For each non-archived account: call `account_repo.compute_balance(id)`.
  2. Separate into assets and liabilities per the `is_liability` rule.
  3. Call `compute_monthly_cashflow(txns, 1)` for current-month income/expenses.
  4. Call `compute_health_score(input)` for the composite score.
  5. Query `recurring_repo.list_upcoming(portfolio_id, 7_days)` for bill count.
  6. Return assembled `DashboardSummary`.

### NetWorthDeltaCalculator

- `compute_delta(portfolio_id, period: DeltaPeriod) -> NetWorthDelta`
  1. Compute today's net worth via `compute_balance` per account (as above).
  2. Compute reference-date net worth via `compute_net_worth_series(ref_date, ref_date)`.
  3. Return delta.

Note: the reference-date balance uses the monthly-snapshot series intentionally —
the delta is "vs. 30 trading days ago" not "vs. real-time yesterday", which smooths
out same-day volatility.

### SafeToSpendCalculator

- `compute(portfolio_id, month) -> SafeToSpend`
  1. Get trailing 3-month average income → `monthly_income_estimate`.
  2. Sum positive transactions received MTD → `income_received_mtd`.
  3. Sum confirmed recurring expenses with `next_expected_date` in the remainder of
     the month → `remaining_obligations`.
  4. Sum savings goal monthly contributions → `goal_contributions`.
  5. Compute and return `SafeToSpend`.

### CategoryPaceCalculator

- `compute_pace(portfolio_id, window: TimeWindow) -> Vec<CategoryHealthIndicator>`
  1. Compute MTD spending per category.
  2. Compute trailing 3-month average per category.
  3. Project month-end from MTD / (days elapsed / days in month).
  4. Classify pace label.
  5. Return top N indicators sorted by `mtd_spent` descending.

### AlertAggregator

- `collect(portfolio_id) -> Vec<Alert>`
  1. Overdue obligations: `confirmed recurring WHERE next_expected_date < today`.
  2. Overspent categories: `mtd_spent > monthly_avg * 1.10`.
  3. Unusual spending: single transactions exceeding 3 standard deviations from
     category mean (future).
  4. Goal milestones: savings goals crossing 25%, 50%, 75%, 100% of target.
  5. Return ordered by priority (1 highest).

---

## 5. API Endpoints (current and planned)

| Method | Path                                                             | Status                      | Returns                        |
| ------ | ---------------------------------------------------------------- | --------------------------- | ------------------------------ |
| `GET`  | `/api/dashboard/summary`                                         | Implemented (fixed ADR-013) | `DashboardSummary`             |
| `GET`  | `/api/dashboard/net-worth?months=N`                              | Implemented                 | `Vec<NetWorthEntry>`           |
| `GET`  | `/api/dashboard/cashflow?months=N`                               | Implemented                 | `Vec<CashFlowEntry>`           |
| `GET`  | `/api/dashboard/spending?month=\|window=`                        | Extend window param         | `Vec<SpendingEntry>`           |
| `GET`  | `/api/dashboard/health-score`                                    | Implemented (fixed ADR-013) | `HealthScore`                  |
| `GET`  | `/api/dashboard/net-worth-delta?period=30d\|90d\|ytd`            | **Planned**                 | `NetWorthDelta`                |
| `GET`  | `/api/dashboard/safe-to-spend`                                   | **Planned**                 | `SafeToSpend`                  |
| `GET`  | `/api/dashboard/category-pace?window=mtd\|rolling30\|last_month` | **Planned**                 | `Vec<CategoryHealthIndicator>` |
| `GET`  | `/api/dashboard/alerts`                                          | **Planned**                 | `Vec<Alert>`                   |

---

## 6. Context Map Integration

```text
┌──────────────────────────────────────────────────────────────────────┐
│                 DASHBOARD PRESENTATION (DDD-007)                      │
│                                                                        │
│  Reads from:                                                           │
│    ┌─────────────────┐   compute_balance      ┌──────────────────┐   │
│    │ Portfolio Mgmt  │ ───────────────────────▶│ DashboardAssem-  │   │
│    │ (DDD-002)       │                         │ bler             │   │
│    │ AccountRepo     │                         │                  │   │
│    └─────────────────┘                         │ NetWorthDelta-   │   │
│                                                │ Calculator       │   │
│    ┌─────────────────┐   txns for analysis     │                  │   │
│    │ Tx Ingestion    │ ───────────────────────▶│ SafeToSpend-     │   │
│    │ (DDD-003)       │                         │ Calculator       │   │
│    │ TransactionRepo │                         │                  │   │
│    └─────────────────┘                         │ CategoryPace-    │   │
│                                                │ Calculator       │   │
│    ┌─────────────────┐   recurring groups      │                  │   │
│    │ Intelligence    │ ───────────────────────▶│ AlertAggregator  │   │
│    │ (DDD-004)       │                         │                  │   │
│    │ RecurringRepo   │                         └──────────────────┘   │
│    └─────────────────┘                                   │           │
│                                                           ▼           │
│    ┌─────────────────┐   HealthScore, cashflow ┌──────────────────┐  │
│    │ Financial       │ ───────────────────────▶│ API Handlers     │  │
│    │ Analysis        │                         │ (dashboard.rs)   │  │
│    │ (DDD-005)       │                         └────────┬─────────┘  │
│    └─────────────────┘                                  │            │
└─────────────────────────────────────────────────────────┼────────────┘
                                                          │
                                                          ▼
                                              ┌───────────────────────┐
                                              │ Frontend              │
                                              │ DashboardPage.tsx     │
                                              │ + Widget components   │
                                              └───────────────────────┘
```

---

## 7. Anti-Corruption Layer Notes

- The dashboard assembly layer must **never** expose internal `TransactionForAnalysis`
  structures to the frontend. All data is projected into named, versioned API response
  types before serialisation.
- `savings_rate` from `HealthScore` is a `f64` in [0.0, 1.0]. The frontend must
  multiply by 100 before displaying as a percentage. This is a display responsibility
  — the domain value is a ratio, not a percentage.
- `spending_trend` from `HealthScore` is an `i8` ordinal (−1/0/1). It must be
  mapped to a categorical label ("Decreasing"/"Stable"/"Increasing") in the UI layer
  and never displayed as a numeric percentage.
