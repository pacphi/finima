# finima-analysis

Pure-function financial analysis engine covering cashflow, net worth, budgets, recurring detection, health scoring, and flow analytics.

## Purpose

This crate contains all financial computation logic for Finima. It operates as a stateless analysis library: every public function takes data as input parameters and returns computed results, with no database or network access. The API crate fetches data from repositories and passes it into these functions for processing.

## Key Types / Modules

| Module            | Description                                                                                                                                                                                                                                                                                                                                                                      |
| ----------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `cashflow.rs`     | `compute_monthly_cashflow()` -- aggregates transactions into monthly inflow/outflow/net summaries; returns `Vec<MonthlyCashFlow>`                                                                                                                                                                                                                                                |
| `net_worth.rs`    | `compute_net_worth_series()` -- computes time-series net worth from account snapshots; returns `Vec<NetWorthPoint>` from `Vec<AccountSnapshot>`                                                                                                                                                                                                                                  |
| `budget.rs`       | `compute_budget_vs_actual()` -- compares budgeted amounts against actual spending by category; `auto_suggest_budgets()` -- generates budget suggestions from historical spending; types: `BudgetEntry`, `BudgetVsActual`, `BudgetSuggestion`                                                                                                                                     |
| `recurring.rs`    | `detect_recurring()` and `RecurringDetector` -- identifies recurring transaction groups by merchant/amount pattern matching; types: `RecurringGroupCandidate`, `TransactionForAnalysis`                                                                                                                                                                                          |
| `health_score.rs` | `compute_health_score()` -- produces a 0-100 financial health score from income, expenses, savings rate, and debt ratios; types: `HealthScore`, `HealthScoreInput`                                                                                                                                                                                                               |
| `flows.rs`        | `detect_flows()` -- identifies money flow patterns between accounts; `build_sankey_data()` -- generates Sankey diagram nodes and links; `build_outflow_ranking()` -- ranks outflow destinations; `build_waterfall()` -- waterfall chart data; types: `FlowCandidate`, `FlowRecord`, `SankeyData`, `SankeyNode`, `SankeyLink`, `OutflowRank`, `WaterfallData`, `WaterfallSegment` |

## Dependencies

Depends on **finima-core** for domain model types (`AccountType`, `Frequency`, etc.) and `Decimal` usage conventions. Uses `chrono` for date arithmetic, `rust_decimal` for all monetary math, and `serde` for result serialization.

## Developer Top-of-Mind

- **All functions are pure**: they take data in, return results out, and have no side effects. No database calls, no network access, no mutation of shared state. Keep it this way.
- **All monetary arithmetic uses `rust_decimal::Decimal`** -- never `f64`. This is consistent with the rest of the codebase and prevents rounding errors.
- **Good test coverage exists**: each module has unit tests with concrete numeric assertions. When modifying computation logic, run the full test suite and verify expected values.
- **`TransactionForAnalysis`** is a lightweight projection of `Transaction` used by the recurring detector and cashflow analyzer. It avoids pulling full domain models into analysis functions.
- When adding new analysis functions, follow the same pattern: accept data as parameters, return a result struct, and add thorough unit tests with `rust_decimal_macros::dec!` for readable assertions.

## Testing

```sh
cargo test -p finima-analysis
```

All tests are pure unit tests with no external dependencies. They construct input data inline and assert on computed output values. The `rust_decimal_macros` dev-dependency provides the `dec!()` macro for readable decimal literals.

## Function Signatures (Summary)

| Function                   | Input                       | Output                         |
| -------------------------- | --------------------------- | ------------------------------ |
| `compute_monthly_cashflow` | `&[TransactionForAnalysis]` | `Vec<MonthlyCashFlow>`         |
| `compute_net_worth_series` | `&[AccountSnapshot]`        | `Vec<NetWorthPoint>`           |
| `compute_budget_vs_actual` | budgets + transactions      | `Vec<BudgetVsActual>`          |
| `auto_suggest_budgets`     | historical transactions     | `Vec<BudgetSuggestion>`        |
| `detect_recurring`         | `&[TransactionForAnalysis]` | `Vec<RecurringGroupCandidate>` |
| `compute_health_score`     | `HealthScoreInput`          | `HealthScore` (0-100)          |
| `detect_flows`             | transactions + accounts     | `Vec<FlowCandidate>`           |
| `build_sankey_data`        | `&[FlowRecord]`             | `SankeyData`                   |
| `build_outflow_ranking`    | `&[FlowRecord]`             | `Vec<OutflowRank>`             |
| `build_waterfall`          | `&[FlowRecord]`             | `WaterfallData`                |

## Architecture Notes

This crate has no async code and no runtime dependencies beyond data types. It can be compiled and tested independently of the rest of the system. All public functions are deterministic given the same input.
