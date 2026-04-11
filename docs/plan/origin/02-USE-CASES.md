# Finima — Use Cases

**Version:** 1.0 | **Date:** 2026-04-10

---

## UC-01: Passwordless Sign-Up / Sign-In

**Actor:** Unauthenticated User  
**Precondition:** User has a valid email address  
**Trigger:** User navigates to Finima and enters their email

**Main Flow:**

1. User enters email on the sign-in page.
2. System generates a cryptographically random token (32 bytes, URL-safe base64), hashes it (SHA-256), and stores the hash with a 15-minute expiry in `magic_links`.
3. System calls the Resend API to deliver a branded email containing the magic link URL (`/auth/verify?token=<raw_token>&email=<email>`).
4. User clicks the link in their email.
5. System receives the GET request, hashes the provided token, looks up the matching record, validates it is not expired and not already used.
6. If the email matches an existing user, system issues a JWT access token (15 min) and refresh token (7 days). If no user exists, system creates a `users` row and then issues tokens.
7. System marks the magic link as used (`used_at = now()`).
8. Frontend stores tokens and redirects to the dashboard (existing user) or onboarding wizard (new user).

**Alternate Flows:**

- **A1 — Expired token:** System returns 401 with message "Link expired, please request a new one."
- **A2 — Already used token:** System returns 401 with message "Link already used."
- **A3 — Resend delivery failure:** System logs the error; user sees "Email could not be sent, please try again."

**Postcondition:** User is authenticated with a valid session.

---

## UC-02: Onboarding Wizard (New User)

**Actor:** Newly registered User  
**Precondition:** UC-01 completed, user has no portfolios

**Main Flow:**

1. System displays a 3-step onboarding wizard.
2. **Step 1 — Profile:** User enters display name and selects preferred currency and date format.
3. **Step 2 — Portfolio:** User creates a portfolio (e.g., "My Finances" or "Johnson Household"). Name is required; description is optional.
4. **Step 3 — First Account:** User adds their first account — selects type from a dropdown (checking, savings, credit card, etc.), enters institution name, account nickname, and opening balance.
5. System saves all entities and redirects to the account detail view with a prompt to upload a transaction file.

**Alternate Flows:**

- **A1 — Skip account:** User can skip Step 3 and land on an empty dashboard. A persistent CTA encourages them to add their first account.

---

## UC-03: Create Account

**Actor:** Authenticated User  
**Precondition:** At least one portfolio exists

**Main Flow:**

1. User navigates to Accounts page and clicks "Add Account."
2. System displays a form: portfolio selector (if multiple), account type, name, institution, currency, opening balance, notes.
3. User fills in the form and submits.
4. System validates inputs (name required, balance numeric, type in allowed enum) and creates the account.
5. System redirects to the new account's detail view.

---

## UC-04: Upload Transaction File

**Actor:** Authenticated User  
**Precondition:** At least one account exists

**Main Flow:**

1. User navigates to an account's detail page and clicks "Import Transactions."
2. System presents a drag-and-drop upload zone accepting `.csv`, `.tsv`, `.ofx`, `.qfx`, `.qbo`, `.qif`, `.xls`, `.xlsx`.
3. User drops or selects a file.
4. System detects file type from extension and magic bytes:
   - **OFX/QFX/QBO:** Parses XML/SGML, extracts `<STMTTRN>` records. Returns preview with date, amount, description, type, memo.
   - **QIF:** Parses line records. Returns preview with date, amount, payee, memo, category (if present).
   - **CSV/TSV/XLS/XLSX:** Returns first 20 rows and column headers. Presents a column-mapping UI where user assigns: date column, amount column, description column, and optionally memo/category/type columns.
5. System shows a preview table. User reviews and confirms.
6. System inserts transactions, computing `dedup_hash = SHA-256(date || amount || description)`. Duplicates are flagged for user decision (skip or import anyway).
7. System queues an LLM categorization job for uncategorized transactions.
8. WebSocket pushes progress updates (parsed: N, imported: M, duplicates: D, categorizing: P%).

**Alternate Flows:**

- **A1 — Parse error:** If file is malformed, system returns a descriptive error ("Row 47: date field empty") and does not import.
- **A2 — Large file (>50K rows):** System processes in background; user sees a progress indicator on the uploads page.

---

## UC-05: LLM Transaction Categorization

**Actor:** System (triggered by UC-04 or manual re-categorize)  
**Precondition:** Transactions exist with `category = NULL`

**Main Flow:**

1. System batches up to 20 transactions per LLM call.
2. For each batch, system constructs a prompt containing:
   - The `categorize_transaction` tool definition (JSON schema).
   - Any user-specific override patterns from `user_category_overrides` as few-shot examples.
   - The batch of transactions (date, amount, description).
3. System sends the prompt to Gemma 4 (via Ollama `/api/chat` or llama.cpp bindings) with `tools` parameter.
4. Model returns structured JSON tool-call responses with category, subcategory, merchant_name, and confidence for each transaction.
5. System updates each transaction. Those with `confidence >= 0.7` are marked as categorized. Those below threshold are flagged `needs_review = true`.
6. System emits a WebSocket event with categorization progress and completion status.

**Alternate Flows:**

- **A1 — LLM timeout/error:** System retries once after 5 seconds. If still failing, marks batch as `categorization_failed` and notifies user.
- **A2 — Model unavailable:** System queues the job and alerts user that LLM is offline; transactions remain uncategorized until model is available.

---

## UC-06: User Category Override (Single)

**Actor:** Authenticated User  
**Precondition:** Transaction exists

**Main Flow:**

1. User views a transaction in the detail or table view.
2. User clicks the category field, which becomes an editable dropdown with autocomplete.
3. User selects a new category and optionally a subcategory.
4. System updates the transaction, sets `user_overridden = true`.
5. System prompts: "Apply this category to all transactions from [merchant_name]?" If yes, system creates a `user_category_overrides` entry and retroactively updates matching transactions.

---

## UC-07: Bulk Category Override

**Actor:** Authenticated User  
**Precondition:** Multiple transactions exist

**Main Flow:**

1. User selects multiple transactions via checkboxes in the table view.
2. User clicks "Bulk Edit" → "Change Category."
3. System presents a category/subcategory selector.
4. User selects the target category and confirms.
5. System updates all selected transactions, sets `user_overridden = true`.

---

## UC-08: View Dashboard

**Actor:** Authenticated User  
**Precondition:** Portfolio with at least one account and imported transactions

**Main Flow:**

1. User navigates to the Dashboard.
2. System queries aggregate data:
   - Net worth = sum of all non-archived account balances.
   - Cash flow = monthly income minus expenses for the last 12 months.
   - Spending breakdown = current month transactions grouped by category.
   - Budget progress = budget limits vs. actual spending.
   - Upcoming bills = next 30 days of predicted recurring payments.
   - Financial health score = composite of savings rate, debt ratio, emergency fund, spending trend.
3. System renders widgets in the user's saved layout (or default layout for new users).
4. Each widget is interactive: clicking a chart segment drills into filtered transaction views.

---

## UC-09: Recurring Payment Detection

**Actor:** System (triggered post-import or on-demand)  
**Precondition:** Account has >= 30 days of transaction history

**Main Flow:**

1. System groups transactions by normalized merchant name (using LLM-assigned `merchant_name`).
2. For each group with >= 2 transactions, system computes inter-transaction intervals.
3. System classifies frequency by matching intervals to known patterns (daily through annual, with tolerances defined in PRD §4.5).
4. System creates or updates `recurring_groups` records with average amount, frequency, next expected date, and metadata.
5. System enriches each group via LLM: full merchant name, whether it's a subscription/bill/income, estimated annual cost.
6. User is notified of newly detected recurring items via a badge on the Recurring page.

---

## UC-10: Manage Budget

**Actor:** Authenticated User

**Main Flow:**

1. User navigates to the Budget page.
2. System displays categories with current month budget limits and actual spending (progress bars).
3. User clicks "Edit Budget" on a category row.
4. User enters a monthly limit and toggles rollover on/off.
5. System saves the budget entry.
6. Optionally, user clicks "Auto-Suggest" — system uses 3-month spending average per category to propose limits, which user can accept or adjust.

---

## UC-11: Create Savings Goal

**Actor:** Authenticated User

**Main Flow:**

1. User navigates to Savings Goals and clicks "New Goal."
2. User enters: goal name (e.g., "Emergency Fund"), target amount, optional target date, and optionally links a savings account.
3. System creates the goal. If an account is linked, system computes `current_amount` from account balance. Otherwise, user manually updates progress.
4. Dashboard widget shows goal progress with projected completion date based on recent savings rate.

---

## UC-12: Browse Financial News Feed

**Actor:** Authenticated User

**Main Flow:**

1. User navigates to the News/Learn section.
2. System displays a paginated feed of articles fetched from configured RSS/Atom sources.
3. Each article card shows: title, source, date, 2-sentence LLM summary, and a relevance badge (based on user's portfolio composition).
4. Clicking a card opens the article's source URL in a new tab.
5. User can filter by topic: budgeting, investing, taxes, credit, retirement.

---

## UC-13: Customize Theme & Layout

**Actor:** Authenticated User

**Main Flow:**

1. User opens Preferences from the sidebar.
2. **Theme tab:** User selects light/dark/system mode and picks an accent color from a palette or enters a hex code.
3. **Layout tab:** User toggles dashboard widgets on/off and drags them to rearrange. Changes are persisted to `users.preferences` JSONB.
4. **General tab:** User sets date format, currency display, fiscal year start month, and default chart type.
5. System applies changes immediately (no page reload required via reactive state).

---

## UC-14: View Aggregate Transactions

**Actor:** Authenticated User

**Main Flow:**

1. User navigates to the Transactions page.
2. System displays all transactions across all accounts in a unified table.
3. User applies filters: date range, account(s), category, amount range, search text.
4. User toggles between table view and chart view (spending trend, category breakdown).
5. User exports filtered results as CSV.

---

## UC-15: View Account Detail

**Actor:** Authenticated User

**Main Flow:**

1. User clicks an account from the Accounts list.
2. System displays: account summary (current balance, last import date, transaction count), transaction table (sortable, filterable), and account-specific charts (balance over time, monthly income vs. expense).
3. User can inline-edit individual transactions.
4. User can initiate a new file import from this view.

---

## UC-16: Tag Account as Primary Income

**Actor:** Authenticated User  
**Precondition:** At least one account exists

**Main Flow:**

1. User navigates to an account's settings (via Accounts list → ⋮ menu → "Edit Account" or Account Detail → Settings icon).
2. User toggles "Primary Income Account" to ON.
3. System sets `is_primary_income = true` on the account.
4. System triggers a background job to detect inter-account flows originating from this account.
5. A new "Money Flow" widget appears on the dashboard (if not already present).

**Alternate Flows:**

- **A1 — Multiple primary accounts:** User tags a second account as primary. System treats both as income sources in the flow analysis. The Sankey diagram shows two source nodes.

---

## UC-17: View Inter-Account Flow (Sankey / Outflow Ranking)

**Actor:** Authenticated User  
**Precondition:** At least one primary income account tagged, at least 1 month of transactions across 2+ accounts

**Main Flow:**

1. User navigates to the Dashboard or a dedicated "Money Flow" page.
2. System queries `account_flows` for the selected month and constructs:
   - A **Sankey diagram** showing flows from primary account(s) → each destination account, with band width proportional to flow volume.
   - An **outflow ranking table** sorted by total monthly outflow: account name, type, avg monthly outflow, % of income, 3-month trend arrow.
3. User selects a different month from a date picker → chart and table update.
4. User clicks a flow band in the Sankey → drills into the matching transactions between those two accounts.
5. User clicks a row in the outflow ranking → navigates to that account's detail view filtered to the selected month.

**Alternate Flows:**

- **A1 — Unmatched outflows:** Transfers detected from the primary account that couldn't be paired with an inflow in any other imported account are shown as "External / Unknown" in the Sankey and table. A tooltip suggests: "Import statements from this account to see the full picture."
- **A2 — No primary account tagged:** System displays a prompt: "Tag a primary income account to see where your money flows."

---

## UC-18: View Balance Impact Waterfall

**Actor:** Authenticated User  
**Precondition:** Primary income account tagged, 1+ months of flow data

**Main Flow:**

1. User navigates to the "Money Flow" page and selects the "Balance Impact" tab.
2. System renders a **waterfall chart** for each primary account:
   - Starting balance → + Income deposits → − Rent/Mortgage → − Amex autopay → − Savings transfer → − Student loan → ... → Ending balance.
   - Each bar segment is labeled with the destination account name and amount.
3. User hovers a segment → tooltip shows: "Amex Gold: $650.00 (7.9% of income) — 3 autopay transactions."
4. User toggles between months to compare how outflow composition has changed.

---

## UC-19: Manually Link Transfer Pair

**Actor:** Authenticated User  
**Precondition:** Two transactions exist across different accounts

**Main Flow:**

1. User views a transaction in the transaction table (e.g., "TRANSFER TO SAVINGS -$500" in checking).
2. User clicks ⋮ menu → "Link as Transfer."
3. System presents a search/filter UI scoped to other accounts, pre-filtered to transactions within ±2 days and matching amount (±1%).
4. System shows candidate matches. User selects the matching inflow (e.g., "DEPOSIT +$500" in savings).
5. System creates an `account_flows` record linking the two transactions, marked `is_auto_detected = false`, `is_confirmed = true`.
6. Both transactions are re-categorized as "Transfer" if not already.

**Alternate Flows:**

- **A1 — No match found:** User can still create the flow link by manually searching all transactions. If the counterpart transaction hasn't been imported, the system creates a one-sided flow record with `target_transaction_id = NULL`.

---

## UC-20: Create Flow Group

**Actor:** Authenticated User  
**Precondition:** Inter-account flows exist

**Main Flow:**

1. User navigates to the "Money Flow" page → "Flow Groups" tab.
2. User clicks "New Group" and enters a name (e.g., "Housing Costs").
3. User selects flows to include: mortgage payment, property tax transfer, homeowner insurance.
4. System creates a `flow_groups` record and associates the selected `account_flows`.
5. In the Sankey diagram, these flows are optionally collapsed into a single grouped band labeled "Housing Costs."
6. In the outflow ranking, the group appears as a rollup row, expandable to show individual flows.
