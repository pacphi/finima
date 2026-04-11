# DDD-002: Portfolio Management Bounded Context

**Date:** 2026-04-10  
**Crate:** `finima-core` (models) + `finima-db` (persistence) + `finima-api` (handlers)

---

## 1. Purpose

Manages the organizational hierarchy of financial data: portfolios contain accounts, accounts contain transactions. This context owns the structure that all other contexts operate within.

## 2. Ubiquitous Language

| Term                       | Definition                                                                                                                                                                                            |
| -------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Portfolio**              | The top-level container grouping a user's financial accounts (e.g., "Phillipson Household"). A user may have multiple portfolios.                                                                     |
| **Account**                | A financial account at an institution: checking, savings, credit card, investment, loan, etc. Always belongs to one portfolio.                                                                        |
| **Account Type**           | Classification enum: `checking`, `savings`, `credit_card`, `investment_brokerage`, `investment_retirement`, `loan_mortgage`, `loan_auto`, `loan_student`, `loan_personal`, `cash`, `crypto`, `other`. |
| **Opening Balance**        | The balance at the time the account was added to Finima. All computed balances start from this anchor.                                                                                                |
| **Current Balance**        | Computed: `opening_balance + SUM(transactions.amount)`. Never stored directly; always derived.                                                                                                        |
| **Primary Income Account** | An account flagged as a source of regular income (paychecks). Used by the Flow Detection context.                                                                                                     |
| **Archived Account**       | Soft-deleted account. Hidden from active views but retained with all transaction history.                                                                                                             |

## 3. Aggregates

### Portfolio (Aggregate Root)

```text
Portfolio
  id: UUID
  user_id: UUID (FK -> User, ownership)
  name: String (required, 1-100 chars)
  created_at: DateTime
```

**Invariants:**

- A portfolio must have a name.
- A portfolio belongs to exactly one user.
- A user can have at most 10 portfolios (practical limit, not hard constraint initially).
- Deleting a portfolio requires all accounts to be archived first.

### Account (Aggregate Root)

```text
Account
  id: UUID
  portfolio_id: UUID (FK -> Portfolio)
  name: String (required)
  institution: String? (bank/broker name)
  account_type: AccountType (enum)
  currency: CurrencyCode (default "USD")
  opening_balance: Decimal
  is_primary_income: bool (default false)
  is_archived: bool (default false)
  notes: String?
  created_at: DateTime
```

**Invariants:**

- An account must belong to a portfolio.
- `account_type` must be a valid enum variant.
- `opening_balance` defaults to `0.00` if not provided.
- An account cannot be hard-deleted while transactions exist. It can only be archived.
- `is_primary_income` can only be set on account types where income makes sense (checking, savings, cash). Enforcement is advisory, not hard — users may have unusual setups.
- `currency` is stored but all current functionality assumes single-currency (USD). Multi-currency conversion is a future enhancement.

## 4. Domain Events

| Event                   | Triggered By                                  | Consumed By                                                        |
| ----------------------- | --------------------------------------------- | ------------------------------------------------------------------ |
| `PortfolioCreated`      | User creates portfolio (onboarding or manual) | Dashboard (show portfolio selector)                                |
| `AccountCreated`        | User adds account                             | Upload prompt, Dashboard widget updates                            |
| `AccountArchived`       | User archives account                         | Dashboard (exclude from net worth), Flows (exclude from detection) |
| `PrimaryIncomeTagged`   | User toggles `is_primary_income`              | Flow Detection context (trigger flow analysis)                     |
| `PrimaryIncomeUntagged` | User toggles off                              | Flow Detection context (rebuild flow data)                         |

## 5. Services

### PortfolioService

- `create_portfolio(user_id, name) -> Result<Portfolio>`
- `list_portfolios(user_id) -> Result<Vec<Portfolio>>`
- `update_portfolio(portfolio_id, name) -> Result<Portfolio>` (with ownership check)

### AccountService

- `create_account(portfolio_id, params) -> Result<Account>` (with portfolio ownership check)
- `list_accounts(portfolio_id) -> Result<Vec<AccountWithBalance>>` — includes computed current balance.
- `get_account(account_id) -> Result<AccountDetail>` — includes balance, last import date, transaction count.
- `update_account(account_id, params) -> Result<Account>`
- `archive_account(account_id) -> Result<()>` — sets `is_archived = true`.
- `set_primary_income(account_id, is_primary: bool) -> Result<()>` — emits `PrimaryIncomeTagged`/`Untagged`.

## 6. Context Boundaries

**This context provides to other contexts:**

- `Portfolio` and `Account` entities for ownership validation.
- `AccountWithBalance` for dashboard aggregation.
- `is_primary_income` flag consumed by the Flow Detection context.

**This context does NOT know about:**

- Transaction contents, categories, or LLM results.
- Budgets, savings goals, or recurring detection logic.
- How files are parsed or uploaded.

## 7. Key Design Decisions

- **Balance is computed, not stored.** This prevents drift between transaction records and balance. Every balance query runs `opening_balance + SUM(amount)`. For performance with large transaction sets, a materialized view or periodic snapshot may be added later.
- **Soft delete only.** Archiving preserves historical data integrity. An archived account's transactions remain for net worth history and flow analysis.
- **Portfolio is the multi-tenancy boundary within a user.** While most users will have one portfolio, the model supports households where different family members maintain separate portfolio views while sharing an account.
