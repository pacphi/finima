# Plan 10: Dashboard Redesign

**Date:** 2026-04-15  
**Status:** In Progress — Phase 1 complete  
**Related ADRs:** ADR-013, ADR-014, ADR-015, ADR-016  
**Related DDD:** DDD-007

---

## Executive Summary

The Finima dashboard had two confirmed bugs and a significant feature gap relative to
the market standard established by Monarch Money, Copilot, Empower, YNAB, Quicken
Simplifi, PocketGuard, and six other applications (research conducted April 2026).

**Bugs fixed (Phase 1 — complete):**

1. Net worth displayed a start-of-month snapshot ($7,696) instead of today's actual
   balance ($239,742). Root cause: `compute_net_worth_series` snapped to April 1 and
   excluded all April transactions. Fix: `get_summary` and `get_health_score` now call
   `compute_balance` per account (ADR-013).
2. Financial health metrics displayed with wrong formatting: `savings_rate` (a 0–1 float)
   rendered via `.toFixed(0)` showed "1%" instead of "57%"; `spending_trend` (an ordinal
   −1/0/1) showed "+1%" instead of "Increasing". Fix: `HealthScoreGauge.tsx` now
   multiplies rates by 100 and maps the ordinal to a text label.

**Feature gaps (Phases 2–5 — planned):**
Research across 12 apps identified 6 features present in 10 or more competitors that
Finima currently lacks. These are detailed below with implementation specifics.

---

## Market Research Summary

_Full report: available in conversation context, April 2026._

### What every competitive app does (table stakes)

| Feature                                           | Finima today                                  | Gap     |
| ------------------------------------------------- | --------------------------------------------- | ------- |
| Live net worth with historical chart              | ✓ (chart exists; live balance now fixed)      | Closed  |
| Net worth 30-day delta (dollar + %)               | ✗                                             | Open    |
| MTD spending by category                          | ✓                                             | —       |
| MTD spending vs prior month comparison            | ✗                                             | Open    |
| Upcoming bills 7-day lookahead                    | Partial (30-day lookahead, no urgency colour) | Partial |
| A "remaining to spend" / safe-to-spend figure     | ✗                                             | Open    |
| Consolidated alerts panel                         | ✗ (none)                                      | Open    |
| Income vs expenses monthly summary (three-number) | ✓ (cashflow chart)                            | —       |

### Key differentiators worth adopting

| App         | Feature                                                  | Research citation                                                                |
| ----------- | -------------------------------------------------------- | -------------------------------------------------------------------------------- |
| Monarch     | Weekly Recap (7-day Δ net worth + spend + upcoming)      | "Users describe this as the right cadence — monthly reviews feel too infrequent" |
| Copilot     | "Free to Spend" = budget surplus after obligations       | Primary health indicator for spending-first users                                |
| YNAB        | Age of Money metric (days between earning and spending)  | "Gamifiable, single number, unique — users target 30+ days"                      |
| Simplifi    | 4-horizon Watchlist (MTD, avg, YTD, projected month-end) | Most precise multi-temporal category display in the set                          |
| PocketGuard | Safe-to-Spend as primary headline number                 | "Most praised feature across budget app reviews"                                 |
| NerdWallet  | Credit score with change delta and factor breakdown      | Differentiated by financial health metric type                                   |

### Universal display conventions (must follow)

1. Net worth is always live — never a snapshot with a named cutoff date.
2. Calendar month (the 1st through today) is the default time window for spending.
3. Red = over budget / negative; green = healthy / positive.
4. Spending uses bar charts; net worth uses line/area charts.
5. Subscriptions and recurring payments are first-class objects, not just line items.
6. Categories are the primary organisational unit for spending.

---

## Phase 1: Bug Fixes (Complete — 2026-04-15)

### 1.1 Live balance for dashboard KPIs

**Files changed:**

- `crates/finima-api/src/handlers/dashboard.rs` — `get_summary` and `get_health_score`

**Change:** Both handlers now call `account_repo.compute_balance(acct.id)` per account
(the same SQL-backed computation used by the Accounts page) rather than
`compute_net_worth_series(today, today)` which produced a start-of-month snapshot.

**Verification:** Dashboard net worth now matches the Accounts page balance.

### 1.2 Health score metric display

**Files changed:**

- `frontend/src/components/charts/HealthScoreGauge.tsx`

**Changes:**

- `savings_rate`: `data.savings_rate.toFixed(0)%` → `(data.savings_rate * 100).toFixed(0)%`
- `debt_ratio`: `data.debt_ratio.toFixed(0)%` → `(data.debt_ratio * 100).toFixed(0)%`
- `spending_trend`: `+1%`/`-1%`/`0%` → `"Increasing"`/`"Stable"`/`"Decreasing"` via
  a `trendLabel` variable
- `summaryText` (ARIA): Updated to match

---

## Phase 2: Net Worth Delta and Sync Freshness

**Priority:** High — present in all 12 surveyed apps  
**ADR:** ADR-013, ADR-014

### 2.1 Backend: `/api/dashboard/net-worth-delta`

New endpoint returning `NetWorthDelta` (see DDD-007):

```rust
// GET /api/dashboard/net-worth-delta?period=30d
pub struct NetWorthDeltaResponse {
    pub current: Decimal,
    pub prior: Decimal,
    pub delta_amount: Decimal,
    pub delta_pct: f64,        // None if prior = 0
    pub reference_period: String,  // "30d", "90d", "ytd"
}
```

Implementation:

1. `current` = live net worth via `compute_balance` per account.
2. `prior` = `compute_net_worth_series(ref_date, ref_date).first().total`.
   - For `30d`: `ref_date = today − 30 days`
   - For `90d`: `ref_date = today − 90 days`
   - For `ytd`: `ref_date = Jan 1 of current year`
3. `delta_amount = current − prior`, `delta_pct = delta / prior.abs() * 100`.

### 2.2 Frontend: Enhance net worth summary card

Update the `SummaryCard` for Net Worth to display:

- Primary: `$239,742` (large, as today)
- Secondary: `+$4,200 (1.8%) this month` — from the delta endpoint
- Tertiary: `synced 3 min ago` — from `last_import` timestamp on the account

```tsx
// Proposed structure (not implemented)
<SummaryCard label="Net Worth" value={formatCurrency(summary.net_worth)} delta={delta?.delta_amount} deltaPct={delta?.delta_pct} freshness={mostRecentImport} />
```

---

## Phase 3: Upcoming Obligations — 7-Day Lookahead with Urgency

**Priority:** High — present in 10 of 12 apps  
**ADR:** ADR-016

### 3.1 Backend changes

The existing `GET /api/recurring?upcoming=true&days=30` endpoint returns a 30-day
window without urgency classification. Change the default `days` to 7 and add an
`urgency` field to the response:

```rust
pub enum Urgency { Overdue, Today, Soon, Upcoming }

pub struct UpcomingObligationResponse {
    // existing fields...
    pub urgency: Urgency,  // computed from days_until_due
}
```

### 3.2 Frontend: Urgency colour coding in the Bills widget

The existing `bills` widget in `DashboardPage.tsx` renders a plain list. Add:

- Left border colour keyed to urgency: red (Overdue/Today), yellow (Soon), green (Upcoming)
- "Today" badge for same-day obligations
- "Overdue" badge for past-due obligations

---

## Phase 4: Safe to Spend

**Priority:** High — present in Copilot, PocketGuard, Simplifi, YNAB, Monarch  
**ADR:** ADR-016  
**DDD:** DDD-007 SafeToSpendCalculator

Research citation:

> "Following PocketGuard's model, compute: (expected remaining income this month) minus
> (remaining bills this month) minus (savings goal contributions) = discretionary
> available. Show this as a single large number with a one-line explanation 'after bills
> and goals.'"
> — Dashboard UX Research Report, April 2026

### 4.1 Backend: `/api/dashboard/safe-to-spend`

```rust
pub struct SafeToSpendResponse {
    pub discretionary: Decimal,
    pub expected_remaining_income: Decimal,
    pub remaining_obligations: Decimal,
    pub goal_contributions: Decimal,  // 0 until goals feature is built
    pub as_of: DateTime<Utc>,
}
```

Implementation (see DDD-007 SafeToSpendCalculator):

1. 3-month average monthly income → `monthly_income_estimate`
2. Sum positive transactions received MTD → `income_received`
3. Sum confirmed recurring expenses with `next_expected_date` before month-end → `remaining_obligations`
4. `discretionary = (monthly_income_estimate − income_received) − remaining_obligations`

### 4.2 Frontend: `safe-to-spend` widget (new, optional)

A new widget card:

```
Safe to Spend
$4,821
after bills and goals

Income remaining est.  +$6,200
Bills remaining           -$1,379
──────────────────────────────
Available now             $4,821
```

Display `$4,821` in green if positive, red if negative.

---

## Phase 5: Category Pace Indicators

**Priority:** Medium — present in Simplifi (Watchlists), Copilot, Monarch  
**ADR:** ADR-016  
**DDD:** DDD-007 CategoryPaceCalculator

Research citation:

> "For the top 5 spending categories MTD, show: category name, amount spent, implied
> month-end projection at current pace, and a small bar showing pace vs prior month
> average. A pace indicator ('on track' / 'running 18% hot') surfaced here prevents
> the need to navigate to a full report."
> — Dashboard UX Research Report, April 2026

### 5.1 Backend: `/api/dashboard/category-pace?window=mtd|rolling30|last_month`

```rust
pub struct CategoryPaceResponse {
    pub category: String,
    pub mtd_spent: Decimal,
    pub monthly_avg: Decimal,           // trailing 3-month average
    pub projected_eom: Decimal,         // mtd_spent / (days_elapsed / days_in_month)
    pub pace_ratio: f64,                // projected_eom / monthly_avg
    pub pace_label: String,             // "OnTrack", "RunningHot", "RunningLow"
}
```

Window parameter changes the date range for `mtd_spent` only; `monthly_avg` always uses
trailing 3 months regardless of window.

### 5.2 Frontend: Enhance spending widget

Augment the existing `SpendingDonut` widget (or add a companion card):

- Per category: show MTD amount + projected EOM + pace label
- RunningHot: orange indicator
- OnTrack: neutral
- RunningLow: green indicator (underspending may be good)

---

## Phase 6: Consolidated Alerts Panel

**Priority:** Medium — YNAB's Spotlight (March 2025) was the most praised redesign of the year  
**DDD:** DDD-007 AlertAggregator

Research citation:

> "Show alerts in a single ordered list: (1) overdue bills, (2) overspent categories,
> (3) unusual spending (anomaly detection), (4) goal milestones, (5) informational.
> Do not scatter alerts across widgets — consolidation reduces alert fatigue."
> — Dashboard UX Research Report, April 2026

### 6.1 Backend: `/api/dashboard/alerts`

```rust
pub enum AlertPriority { P1_Overdue, P2_Overspent, P3_Unusual, P4_Milestone, P5_Info }

pub struct Alert {
    pub id: Uuid,
    pub priority: AlertPriority,
    pub title: String,
    pub body: String,
    pub action_url: Option<String>,  // e.g. "/transactions?category=dining"
    pub created_at: DateTime<Utc>,
}
```

Populate from:

1. `recurring_repo.list_overdue(portfolio_id)`
2. MTD category spend > monthly_avg × 1.10
3. (Future) anomaly detection on individual transactions
4. (Future) savings goal milestones

### 6.2 Frontend: Alerts panel widget

A new `alerts` widget (optional, default on):

- Renders `Vec<Alert>` ordered by priority then recency
- Each alert has a dismiss button (local state only in v1)
- Zero-alert state: "You're all caught up" in muted text

---

## Phase 7: Time-Window Switcher

**Priority:** Low — enhancement, not table stakes  
**ADR:** ADR-016

Add a three-position toggle to the spending widget, category pace widget, and budget
vs actual widget:

```tsx
<TimeWindowSwitcher value={timeWindow} onChange={setTimeWindow} options={['mtd', 'rolling_30', 'last_month']} labels={['This Month', 'Trailing 30', 'Last Month']} />
```

Each affected widget re-fetches with the new window on change. The switcher is
positioned in the widget header, consistent styling across all three affected widgets.

---

## Implementation Sequence

| Phase                          | Priority | Effort                                    | Status      |
| ------------------------------ | -------- | ----------------------------------------- | ----------- |
| 1. Bug fixes                   | Critical | Done                                      | ✅ Complete |
| 2. Net worth delta + freshness | High     | Small (1 endpoint + UI)                   | Pending     |
| 3. Obligation urgency          | High     | Small (enum + UI colour)                  | Pending     |
| 4. Safe to Spend               | High     | Medium (new calc + widget)                | Pending     |
| 5. Category pace               | Medium   | Medium (new endpoint + UI)                | Pending     |
| 6. Alerts panel                | Medium   | Medium (aggregator + widget)              | Pending     |
| 7. Time-window switcher        | Low      | Small (UI only, backend already supports) | Pending     |

---

## Non-Goals (explicitly out of scope for this plan)

- Investment account integration (fee analyzer per Empower — requires investment data model)
- Age of Money metric (requires tracking income receipt timing — significant data model change)
- Credit score display (requires third-party API integration)
- Server-side layout persistence (widget positions synced across devices)
- ML-based anomaly detection for unusual spending (no training data yet)
- Weekly Recap push notification (requires notification infrastructure)

---

## References

1. Dashboard UX Research Report, conducted April 2026 — Monarch, Copilot, YNAB, Empower, Simplifi, Tiller, PocketGuard, NerdWallet, Honeydue, Buddy, Wealthfront, Betterment.
2. ADR-013: Live Account Balance for All Dashboard Summary KPIs
3. ADR-014: Net Worth as Primary Dashboard Organizing Frame
4. ADR-015: Widget-Based Customisable Dashboard Canvas
5. ADR-016: Calendar Month as Default Time Window with Three-Position Switcher
6. DDD-007: Dashboard Presentation Bounded Context
7. `crates/finima-analysis/src/health_score.rs` — `compute_health_score`
8. `crates/finima-analysis/src/net_worth.rs` — `compute_net_worth_series`
9. `crates/finima-api/src/handlers/dashboard.rs` — all dashboard handlers
10. `frontend/src/components/charts/HealthScoreGauge.tsx` — health metric display
