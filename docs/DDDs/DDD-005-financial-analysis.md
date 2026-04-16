# DDD-005: Financial Analysis Bounded Context

**Date:** 2026-04-10  
**Crate:** `finima-analysis` + `finima-api` (dashboard handlers)

---

## 1. Purpose

Computes all derived financial insights: net worth, cash flow, spending breakdowns, budget vs. actual comparisons, financial health scores, inter-account flow detection, and flow visualization data (Sankey, waterfall, outflow ranking). This context reads transaction data produced by Ingestion and Intelligence and transforms it into actionable analytics.

## 2. Ubiquitous Language

| Term                       | Definition                                                                                                              |
| -------------------------- | ----------------------------------------------------------------------------------------------------------------------- |
| **Net Worth**              | Sum of all non-archived account balances (assets positive, liabilities negative) at a point in time.                    |
| **Cash Flow**              | Income minus expenses for a given period (typically monthly).                                                           |
| **Spending Breakdown**     | Transactions grouped by category for a period, showing amount and percentage of total expenses.                         |
| **Budget**                 | A user-set monthly spending limit for a category.                                                                       |
| **Budget vs. Actual**      | Comparison of budget limits to actual spending, expressed as amount remaining and percentage consumed.                  |
| **Rollover**               | Unspent budget from the previous month carried forward to the current month (per user preference).                      |
| **Savings Goal**           | A named target amount with optional target date and linked account. Progress is tracked as current vs. target.          |
| **Financial Health Score** | A composite 0-100 metric combining savings rate, debt-to-income ratio, emergency fund coverage, and spending trend.     |
| **Account Flow**           | A detected transfer of money from one account to another, matched by amount (+-1%) and date (+-2 days).                 |
| **Flow Group**             | A user-defined grouping of related flows (e.g., "Housing Costs" = mortgage + property tax + insurance).                 |
| **Sankey Data**            | Aggregated source -> target -> amount per month, used to render a Sankey diagram.                                       |
| **Outflow Ranking**        | Destination accounts sorted by total monthly outflow from primary income accounts.                                      |
| **Balance Impact**         | A waterfall computation: starting balance + income - outflow_1 - outflow_2 - ... = ending balance, per primary account. |

## 3. Aggregates

### Budget (Entity)

```text
Budget
  id: UUID
  portfolio_id: UUID
  category: String
  monthly_limit: Decimal
  rollover: bool
  month: Date (first of month, e.g., 2026-04-01)
```

**Invariants:**

- One budget per (portfolio, category, month).
- `monthly_limit` must be positive.
- If `rollover = true`, the effective limit for a month = `monthly_limit + unspent_from_previous_month`.

### SavingsGoal (Aggregate Root)

```text
SavingsGoal
  id: UUID
  portfolio_id: UUID
  name: String
  target_amount: Decimal
  current_amount: Decimal
  target_date: Date?
  linked_account_id: UUID? (FK -> Account)
```

**Invariants:**

- `target_amount` must be positive.
- If `linked_account_id` is set, `current_amount` is derived from the linked account's balance (not manually editable).
- If no linked account, `current_amount` is manually updated by the user.

### AccountFlow (Entity)

```text
AccountFlow
  id: UUID
  portfolio_id: UUID
  source_account_id: UUID
  target_account_id: UUID
  source_transaction_id: UUID?
  target_transaction_id: UUID?
  amount: Decimal
  flow_date: Date
  is_auto_detected: bool
  is_confirmed: bool
  flow_group_id: UUID?
```

**Invariants:**

- `source_account_id != target_account_id`.
- `amount` must be positive (flow direction is implied by source -> target).
- Auto-detected flows start with `is_confirmed = false` until user reviews.
- A flow may be one-sided (`target_transaction_id = NULL`) when the target account hasn't been imported.

### FlowGroup (Aggregate Root)

```text
FlowGroup
  id: UUID
  portfolio_id: UUID
  name: String
```

**Invariants:**

- Names are unique per portfolio.
- A flow can belong to at most one group.

## 4. Domain Services

### NetWorthCalculator

- `compute_time_series(portfolio_id, start_date, end_date) -> Vec<(Date, Decimal)>`
  - For each date in range, sum all account balances (opening_balance + transactions up to that date).
  - Credit cards and loans contribute negative values.

### CashFlowCalculator

- `compute_monthly(portfolio_id, months: usize) -> Vec<MonthCashFlow { month, income, expenses, net }>`
  - Group transactions by month, separate income (positive) from expenses (negative).

### SpendingAnalyzer

- `breakdown_by_category(portfolio_id, month) -> Vec<CategorySpend { category, amount, percentage }>`
  - Aggregate expenses by category for the given month.

### BudgetEngine

- `get_vs_actual(portfolio_id, month) -> Vec<BudgetVsActual { category, limit, spent, remaining, pct }>`
  - Join budgets with spending breakdown. Apply rollover if enabled.
- `auto_suggest(portfolio_id) -> Vec<BudgetSuggestion { category, suggested_limit }>`
  - Compute 3-month average per category, rounded to nearest $25.

### HealthScorer

- `compute(portfolio_id) -> HealthScore { score: u8, savings_rate, debt_ratio, emergency_months, spending_trend }`
  - **Savings rate:** (income - expenses) / income for last 3 months.
  - **Debt ratio:** total liabilities / total assets.
  - **Emergency fund months:** liquid savings / average monthly expenses.
  - **Spending trend:** is total spending increasing, stable, or decreasing over 3 months?
  - Composite score: weighted average mapped to 0-100.

### FlowDetector

- `detect_flows(portfolio_id) -> Vec<AccountFlowCandidate>`
  1. Get all primary income accounts for the portfolio.
  2. For each outflow from a primary account, search for a matching inflow in other accounts: same amount (+-1%), date within +-2 days.
  3. Create `AccountFlow` records for matches.
  4. For unmatched outflows with transfer-like descriptions (detected by keyword or LLM), create one-sided flow records.

### SankeyDataBuilder

- `build(portfolio_id, month) -> SankeyData { nodes: Vec<Node>, links: Vec<Link { source, target, value }> }`
  - Aggregate flows by source_account -> target_account for the month.
  - Collapse flow groups into single nodes if user has configured groups.

### OutflowRanker

- `rank(portfolio_id, month) -> Vec<OutflowRank { account, type, monthly_amount, pct_income, trend }>`
  - Sort destination accounts by total monthly outflow.
  - Compute 3-month trend (increasing/stable/decreasing).

### WaterfallBuilder

- `build(account_id, month) -> WaterfallData { start_balance, income, outflows: Vec<(account_name, amount)>, end_balance }`
  - Starting balance = balance at start of month.
  - Income = sum of positive transactions.
  - Outflows = flows to each destination account.
  - End balance = start + income - sum(outflows).

### FullSankeyBuilder

- `build(portfolio_id, month) -> FullSankeyData { nodes: Vec<Node>, links: Vec<Link>, metadata: SankeyMetadata }`
  - Composes four columns into a single Sankey dataset (see ADR-008 Amendment 2):
    1. **Income inflows (column `left`):** Positive transactions on primary income accounts, grouped by merchant/category. Excludes transactions already in `account_flows` or with `category = 'transfer'`.
    2. **Primary accounts (column `primary`):** Accounts with `is_primary_income = true` — the paycheck hub.
    3. **Secondary accounts (column `secondary`):** Non-primary accounts (credit cards, savings, loans), plus **spender-role virtual nodes** synthesized when a primary account has direct spending. The virtual node for primary `Joint` is named `"Joint — Direct Debit"` and carries `node_type: "spender_role"`. Inserting it (Sugiyama dummy-node technique) keeps every spending edge a single-column step.
    4. **Spending categories (column `right`):** Categories receiving outflow transactions. Aggregated by `(source_account_or_spender, category)` pairs.
  - Spending source-of-truth: `transactions.direction = 'outflow'` (set at import time by `SignNormalizer`; see ADR-018). Replaces the old account-type-conditional sign branch which silently misclassified Chase-issued credit cards.
  - Excluded from spending: rows whose `category` is in `config.sankey.transfer_categories` (default `["transfer", "debt_payment"]`) and rows whose `id` appears in `account_flows`.
  - Link classes:
    - **income_links** — `left → primary`
    - **transfer_links** — `primary → secondary` (from `account_flows`)
    - **spender_role_inflows** — `primary → spender_role` (virtual; col 1 → col 2)
    - **spender_role_spending** — `spender_role → right` (virtual; col 2 → col 3)
    - **secondary_spending** — `secondary → right` (col 2 → col 3)
  - Applies a 2% small-value aggregation threshold per column: nodes below the threshold are collapsed into "Other" / "Other Income" buckets.
  - Returns `SankeyMetadata { month, total_income, total_spending, net_flow }`.
  - Consumed by `InteractiveSankey` in the frontend, which renders the diagram with one interaction law: only category leaves are clickable; clicking opens a subcategory donut (no account-level drilling).

## 5. Domain Events

| Event            | Triggered By                                 | Consumed By                      |
| ---------------- | -------------------------------------------- | -------------------------------- |
| `FlowsDetected`  | Flow detection completes after import        | WebSocket, Dashboard flow widget |
| `BudgetExceeded` | Spending exceeds budget for a category       | Dashboard alert, notification    |
| `GoalReached`    | Savings goal current_amount >= target_amount | Dashboard celebration            |

## 6. Context Boundaries

**This context consumes from other contexts:**

- Transaction data (amounts, dates, categories, merchant names) from the shared `transactions` table.
- Account data (balances, types, `is_primary_income`) from Portfolio Management context.
- Recurring groups from Intelligence context (for upcoming bills widget).
- LLM client from Intelligence context (for flow insight generation).

**This context provides to other contexts:**

- Computed analytics data consumed by API handlers for dashboard rendering.
- Flow data consumed by the frontend for Sankey, waterfall, and outflow ranking visualizations.

**This context does NOT know about:**

- File parsing, upload workflows, or column mapping.
- Authentication or session management.
- RSS feeds or article summarization.
