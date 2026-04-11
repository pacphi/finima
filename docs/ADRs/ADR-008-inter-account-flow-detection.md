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
