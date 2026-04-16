# ADR-008: Inter-Account Flow Detection and Visualization

**Status:** Accepted  
**Date:** 2026-04-10  
**Deciders:** Chris Phillipson

---

## Context

A key insight users seek from personal finance tools is: "Where does my paycheck go?" This requires detecting how money flows between accounts — checking to savings, checking to credit card payments, checking to loan payments — and visualizing it intuitively.

No competitor offers this as a first-class feature. Monarch and YNAB focus on category-level budgeting but don't trace inter-account money movement. This is a significant differentiator for Finima.

## Decision

Implement a **multi-layer flow detection and visualization system**:

### Detection Layer

1. **Primary account tagging:** Users flag accounts as `is_primary_income = true` (where paychecks land). Multiple primary accounts supported (household with two earners).

2. **Automatic transfer matching:** Match complementary transactions across accounts:
   - Outflow from Account A on date D for amount X
   - Inflow to Account B on date D (+-2 days) for amount X (+-1% tolerance)
   - Store matches in `account_flows` table with `is_auto_detected = true`.

3. **LLM-assisted matching:** For transactions with descriptions like "TRANSFER TO SAVINGS" or "AUTOPAY AMEX", use the LLM to infer the target account when only one side of the transfer is visible (user hasn't imported all accounts).

4. **One-sided flows:** When a matched inflow account doesn't exist in the system, create a flow record with `target_transaction_id = NULL`. Displayed as "External / Unknown" in visualizations.

### Visualization Layer

- **Sankey diagram:** Primary accounts on the left, destination accounts on the right. Band width proportional to monthly flow volume.
- **Outflow ranking table:** Sorted by monthly outflow amount. Columns: account name, type, average monthly outflow, % of income, 3-month trend arrow.
- **Balance impact waterfall:** For each primary account: starting balance + income - outflow_1 - outflow_2 - ... = ending balance.
- **Flow groups:** User-defined groupings (e.g., "Housing Costs" = mortgage + property tax + insurance). Groups collapse into single bands in Sankey.

### Data Model

```text
account_flows:  source_account_id, target_account_id, source_transaction_id?,
                target_transaction_id?, amount, flow_date, is_auto_detected,
                is_confirmed, flow_group_id?
flow_groups:    portfolio_id, name
```

## Consequences

**Positive:**

- Answers the "where does my money go?" question at the account level, not just the category level.
- Sankey visualization makes invisible autopay drains viscerally clear.
- Flow groups enable meaningful aggregation (all housing costs in one band).
- Trend detection catches creeping expense increases early.
- Strong differentiator — no major competitor offers this.

**Negative:**

- Transfer matching heuristics will produce false positives (coincidental same-amount transactions). Mitigated: user confirmation flow and ability to dismiss false matches.
- Accuracy degrades when users haven't imported all accounts (one-sided flows). Mitigated: clear messaging suggesting additional imports.
- Sankey rendering is not included in Recharts. Requires a custom component or third-party library (e.g., `d3-sankey`).
- Waterfall charts are also not in Recharts. May need custom implementation or a library like `recharts` extended with custom shapes.

## Alternatives Considered

1. **Category-only analysis** — Easier to implement but doesn't answer the inter-account flow question. Most competitors stop here. Rejected as the sole approach; categories are still implemented but flows add a complementary dimension.
2. **Manual-only flow linking** — No auto-detection; user manually links every transfer. Too tedious for users with 5+ accounts and monthly recurring transfers. Rejected as primary approach but supported as a fallback.
3. **Deferred to Phase 2+** — Could ship without flows and add later. Rejected because account flow is a core differentiator listed in the PRD and warrants inclusion from Phase 2 onward.

---

## Amendment: Three-Column Sankey Enhancement

**Status:** Accepted  
**Date:** 2026-04-16

### Context

The original Sankey visualization (ADR-008) shows inter-account transfers: primary income accounts on the left, destination accounts on the right. This answers "where does my paycheck move between accounts?" but stops short of answering the full question: "where does my money come from, and where does it ultimately go?"

Users see transfers to a credit card but not what those credit card charges bought. They see a paycheck landing but not whether it came from one employer or multiple income streams. Extending the Sankey to three columns closes both gaps in a single visualization.

### Decision

Extend the Sankey diagram from a 2-column layout to a **3-column layout**:

```text
LEFT COLUMN            MIDDLE COLUMN           RIGHT COLUMN
(Income Sources)       (User Accounts)         (Spending Categories)

Employer Paycheck ──┐                    ┌──── Groceries
                    ├──> Checking ───────┤
Side Gig Income ────┘        │           ├──── Housing
                             │           └──── Dining
                             ├──> Savings
                             │
                             └──> Credit Card ──┬── Travel
                                                ├── Subscriptions
                                                └── Other
```

#### Left Column: Income Sources

- Aggregates deposit/paycheck transactions flowing INTO primary income accounts.
- Transactions are grouped by merchant or category (e.g., "ACME Corp Payroll", "Freelance Deposits").
- Source: `transactions` table filtered to positive amounts on primary income accounts, excluding transfers already present in `account_flows`.

#### Middle Column: User Accounts

- Connected by inter-account transfer flows (existing `account_flows` behavior from the original ADR-008 decision).
- No changes to detection or matching logic.

#### Right Column: Spending Categories

- Aggregates outflow transactions across ALL accounts, grouped by category.
- Source: `transactions` table filtered to negative amounts (expenses), excluding transactions already present in `account_flows` and those with `category = 'transfer'`.
- Categories come from the existing LLM-enriched category field on transactions.

#### New Endpoint

```text
GET /api/flows/sankey-full?month=YYYY-MM
```

Returns a `FullSankeyResponse` containing:

- `nodes`: All three columns of nodes (income sources, accounts, spending categories).
- `links`: Directed edges with monetary values connecting nodes across adjacent columns.
- `metadata`: Month, total income, total spending, net flow.

#### Transfer Deduplication

To avoid double-counting, the left and right columns exclude:

1. Transactions whose `id` appears as `source_transaction_id` or `target_transaction_id` in the `account_flows` table (already represented as inter-account transfers in the middle column).
2. Transactions with `category = 'transfer'` (heuristic catch-all for transfers not yet matched by the flow detector).

#### Small-Value Aggregation

Nodes representing less than 2% of their column's total are collapsed into an "Other" bucket. This keeps the diagram readable when a user has many small income sources or spending categories. The 2% threshold is applied per-column independently.

### Consequences

**Positive:**

- Answers the complete money flow question end-to-end: income sources -> accounts -> spending.
- Reuses existing infrastructure (flow detector, transaction categories, account data) with no schema changes.
- The 2% aggregation threshold keeps the diagram clean for users with many small transactions.
- Single endpoint (`/sankey-full`) provides all data needed, reducing frontend round-trips.

**Negative:**

- Increases Sankey rendering complexity (three columns vs. two). Frontend must handle horizontal layout with three node groups.
- Deduplication logic adds coupling between the transaction query and the `account_flows` table. If flow detection misses a transfer, it could appear in both the middle column (as an unmatched flow) and the right column (as spending). Mitigated: the `category = 'transfer'` fallback filter catches most of these.
- The "Other" bucket hides detail. Users who want to see all categories must expand it or adjust the threshold. Future enhancement: make the threshold configurable or allow click-to-expand.

## Amendment 2: 4-Column Sankey, Spender-Role Virtual Nodes, and Direction-Based Aggregation

**Status:** Accepted
**Date:** 2026-04-16
**Related:** ADR-018 (Import-Time Sign Normalization)

### Context

Amendment 1 described a 3-column Sankey (income → accounts → categories). In practice, users needed to distinguish the _primary_ income-receiving account (the paycheck hub) from _secondary_ accounts (credit cards, savings, loans) that receive transfers from the hub. Collapsing all accounts into a single middle column obscured the canonical "paycheck → checking → credit card → category" story.

Two additional problems surfaced:

1. **Skip-edge layout artifacts.** When a primary account has direct spending (debit-card, autopay) AND transfers to secondary accounts, the resulting graph had col 1 → col 3 edges that visually crossed col 2 → col 3 ribbons. d3-sankey laid this out, but the result was unreadable.
2. **Institution-dependent sign assumptions.** The original spending-link aggregation hard-coded one sign convention per `account_type`, silently misclassifying spending on Chase-issued credit cards (United Explorer Mastercard, Amazon Prime Visa) where charges export as negative amounts.

### Decision

Three changes, working together:

**1. Split the middle column.** The Sankey now has four columns:

```text
LEFT          PRIMARY        SECONDARY                      RIGHT
(income)      (paycheck hub) (cards / savings / spender-role) (categories)

Salary  ─→ Joint Checking ─┬─→ United Explorer ─┬─→ Travel
                           ├─→ Joint — Direct Debit ─┬─→ Groceries
                           │   (spender-role node)   └─→ Utilities
                           └─→ Discover ─┬─→ Food Dining
                                         └─→ Shopping
```

Backend `GET /api/flows/sankey-full?month=YYYY-MM` returns nodes with `column ∈ {left, primary, secondary, right}` and `node_type ∈ {income, account, spender_role, spending}`. Three link classes:

- **income_links** — left → primary (positive deposit transactions on primary accounts, excluding rows already in `account_flows`).
- **transfer_links** — primary → secondary (from `account_flows`).
- **spending_links** — (primary ∪ secondary ∪ spender_role) → right (transactions where `direction = outflow` and `category ∉ sankey.transfer_categories` and the row is not in `account_flows`).

**2. Spender-role virtual nodes.** When a primary account has direct spending, the backend synthesizes a `"{Primary} — Direct Debit"` node in column 2 (`secondary`) with `node_type: "spender_role"` and routes the spending through it:

```text
Joint (col 1) → Joint — Direct Debit (col 2) → Groceries (col 3)
```

This is the textbook Sugiyama dummy-node insertion technique for layered DAG layout. It eliminates skip edges entirely; every link satisfies `column_index(source) + 1 == column_index(target)` (with the legitimate exception of `primary → secondary`). The frontend renders spender-role nodes with a dashed outline so users see the dual role explicitly: a checking account is _both_ a hub for incoming transfers and a source of direct spending.

**3. Direction-based aggregation.** The spending-link aggregation no longer inspects the sign of `amount` or branches on `account_type`. Instead it queries `direction == Outflow` (set at import time by the `SignNormalizer` service — see ADR-018) and excludes categories listed in `config.sankey.transfer_categories` (default: `["transfer", "debt_payment"]`). One predicate, every account type, every institution.

### Interaction model

| User action                 | Result                                             |
| --------------------------- | -------------------------------------------------- |
| Default view                | Full 4-column DAG, always visible. No drill state. |
| Click income / primary node | No-op. Display-only.                               |
| Click secondary / spender   | No-op. The full chain is already visible.          |
| Click category (right col)  | Subcategory donut opens beside the diagram.        |
| Click category again, or X  | Donut dismisses.                                   |

The interaction law is uniform: **leaves are always categories; only categories are clickable; the only drill is category → subcategory donut.** Account-level drilling, breadcrumbs, navigation stack, and back buttons were removed in the Phase 7 frontend simplification.

### Configuration

`config/sankey.yaml`:

```yaml
sankey:
  transfer_categories:
    - transfer
    - debt_payment
```

The transfer-category list is configurable so installations with custom category schemes can mark additional categories as transfers (e.g., `investment_buy`).

### Consequences

**Positive**

- Single diagram tells the full story without a navigation stack.
- One interaction law replaces three different click semantics.
- ~140 lines of frontend complexity (`buildMiniSankey`, `navStack`, breadcrumbs, secondary-click branch, column remap) deleted.
- `flows.rs` spending block lost ~25 lines and gained one config-driven predicate.
- Spending categories on Chase-issued cards now appear in the diagram (they previously did not — root cause was the sign heuristic, not data quality).
- Strict left-to-right monotonicity makes d3-sankey lay the graph cleanly without overlap.

**Negative**

- One additional virtual node per primary account that has direct spending. Net node count goes up by 0–N where N = primary count. Negligible for personal-finance scale.
- Users who import data through an institution we haven't seen rely on `SignAutodetector` (Phase 5.5 / ADR-018) for the first import. Per-account override (Phase 5.7) catches the rest.

### Supersedes

The "3-column" language in Amendment 1 is superseded by this amendment. The spending column referenced there is now the rightmost of four; spending sources are no longer all in the middle column but split across primary, secondary, and spender-role positions.
