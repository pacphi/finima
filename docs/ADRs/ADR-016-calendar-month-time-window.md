# ADR-016: Calendar Month as Default Time Window with Three-Position Switcher

## Status

Proposed

## Date

2026-04-15

## Context

Time-period framing is one of the highest-impact UX decisions in a personal finance
application. The three most common windows are:

| Window                   | Definition                         | Aligns with                            |
| ------------------------ | ---------------------------------- | -------------------------------------- |
| **Calendar month (MTD)** | 1st of current month through today | Credit card statements, billing cycles |
| **Rolling 30 days**      | Today minus 30 days through today  | Continuous view, no month-end cliff    |
| **Last month**           | Complete prior calendar month      | Completed baseline for comparison      |

Research found this to be the single most consistent convention across all 12 apps
surveyed:

> "Current calendar month is the default time window for spending — not rolling 30 days
> from today — calendar month (1st through today). This convention is universal across
> Monarch, Copilot, YNAB, Simplifi, PocketGuard, and NerdWallet."
> — Dashboard UX Research Report, April 2026

Copilot's "Net This Month" label and YNAB's budget allocation model both explicitly
define their primary window as the calendar month. Simplifi's Watchlist widget goes
further: it displays MTD, monthly average (trailing), YTD, and projected month-end
simultaneously — confirming that multiple time horizons have legitimate value.

Finima's current implementation uses calendar-month for `GET /api/dashboard/spending`
(when a `?month=` param is provided) but the dashboard page always passes the current
month and offers no switcher.

## Decision

**All time-sensitive dashboard widgets default to the current calendar month.**

Additionally, widgets that show period-relative data (spending, cash flow, budget vs
actual, category pace) expose a **three-position switcher**:

```text
[ MTD ]  [ Rolling 30 ]  [ Last Month ]
```

- Selecting a position re-fetches only that widget's data with the appropriate date
  range — no full-page reload.
- The selected position is stored in widget-local React state (not persisted to
  localStorage, as users expect the default on return visits).

### API contract changes required

| Endpoint                           | Current                  | Change needed                                                  |
| ---------------------------------- | ------------------------ | -------------------------------------------------------------- |
| `GET /api/dashboard/spending`      | Accepts `?month=YYYY-MM` | Add `?window=mtd\|rolling30\|last_month` (or accept start/end) |
| `GET /api/dashboard/cashflow`      | Accepts `?months=N`      | No change — bar chart already shows N complete months          |
| `GET /api/dashboard/category-pace` | New endpoint             | Must accept all three windows                                  |

### Time-window computation rules

| Window       | `start_date`           | `end_date`              |
| ------------ | ---------------------- | ----------------------- |
| `mtd`        | first of current month | today                   |
| `rolling30`  | today − 30 days        | today                   |
| `last_month` | first of prior month   | last day of prior month |

## Consequences

### Positive

- Calendar-month default meets user expectations established by every major competitor.
- Three-position switcher gives power users a continuous view without breaking the
  default experience.
- Projected month-end spend (useful for the category pace widget) is only meaningful in
  the MTD window — the switcher model naturally enables this conditional display.

### Tradeoff

- "Rolling 30 days" can produce confusing results mid-month (e.g., 15 April to 15 May
  spans two billing cycles). A tooltip or label — "Trailing 30 days" — sets
  expectations.
- Last month is a completed window; it cannot be mixed with "in-progress" projections.
  The UI should suppress pace/projection indicators when the Last Month window is active.
