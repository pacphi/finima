# ADR-015: Widget-Based Customisable Dashboard Canvas

## Status

Accepted (baseline implemented; widget set to be extended per ADR-016 and this plan)

## Date

2026-04-15

## Context

The Finima dashboard already uses `react-grid-layout` with drag-and-drop widget
repositioning and localStorage persistence (`finima-dashboard-layout`). This was
implemented in a previous iteration. This ADR formalises the widget model and defines
the canonical widget set, promotion/demotion rules, and the contract between widgets and
the backend API.

Research found that customisable widget dashboards are the clear market direction:

> "Monarch's default home screen is a fully customisable widget canvas. Widgets include
> net worth, recent transactions, spending trend, cash flow, investment performance,
> recurring transactions, and the Weekly Recap."
> — Dashboard UX Research Report, April 2026

Copilot's iPad dashboard was specifically praised for showing everything simultaneously
without tab-switching — an outcome that widget-per-screen-region layouts achieve
naturally on larger viewports.

## Decision

### Widget taxonomy

Widgets are classified as **required** or **optional**:

| Widget ID       | Title                | Required?    | Backend endpoint                          |
| --------------- | -------------------- | ------------ | ----------------------------------------- |
| `net-worth`     | Net Worth            | Yes          | `GET /api/dashboard/net-worth?months=12`  |
| `health`        | Financial Health     | Yes          | `GET /api/dashboard/health-score`         |
| `cashflow`      | Cash Flow            | Yes          | `GET /api/dashboard/cashflow?months=6`    |
| `spending`      | Spending by Category | Yes          | `GET /api/dashboard/spending?month=…`     |
| `bills`         | Upcoming             | Yes          | `GET /api/recurring?upcoming=true&days=7` |
| `budget`        | Budget vs Actual     | No           | `GET /api/budgets/vs-actual?month=…`      |
| `safe-to-spend` | Safe to Spend        | No (planned) | `GET /api/dashboard/safe-to-spend`        |
| `category-pace` | Category Health      | No (planned) | `GET /api/dashboard/category-pace`        |
| `goals`         | Savings Goals        | No (planned) | `GET /api/goals`                          |

Required widgets cannot be removed from the layout, only repositioned. Optional widgets
can be hidden via a "Customize" menu (to be implemented).

### Widget contract

Each widget:

- Fetches its own data independently (no single mega-fetch, to allow partial loading).
- Renders a skeleton/loading state while fetching.
- Renders an empty state with a call-to-action if data is absent (e.g., "Set up a
  budget" for the budget widget).
- Exposes an error boundary so one widget failure does not blank the page.

### Layout persistence

The layout JSON stored in `localStorage` under `finima-dashboard-layout` is the source
of truth for user customisation. Future work will persist layout server-side (per user)
to sync across devices.

### Breakpoint behaviour

| Breakpoint      | Columns | Notes                         |
| --------------- | ------- | ----------------------------- |
| lg (≥1024px)    | 12      | Default multi-widget grid     |
| md (768–1023px) | 12      | Same layout, narrower gutters |
| sm (<768px)     | 12      | Widgets stack full-width      |

## Consequences

### Positive

- Users with different financial philosophies (net worth vs spending vs goals) can
  arrange the dashboard to reflect their priorities without us choosing for them.
- New widgets can be added incrementally without disrupting existing layouts.
- Each widget's independent data fetch means the page paints progressively — users see
  net worth before the 12-month cashflow data arrives.

### Tradeoff

- Layout stored in localStorage is lost if the user clears browser data or switches
  devices. Server-side persistence is deferred.
- The per-widget fetch model means N API calls on page load (currently 7). A future
  optimisation can batch these via a single `GET /api/dashboard/all` that returns all
  widget payloads, but this is premature until profiling identifies it as a bottleneck.
