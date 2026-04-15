# User Guide

This guide walks through every feature in Finima. If you have not set
up the application yet, start with the [Quick Start](quick-start.md)
guide first.

---

## Dashboard

The dashboard is your home screen. It displays six widgets that
summarize your finances at a glance. You can rearrange widgets by
dragging the handle at the top of each card, and your layout is saved
automatically in the browser.

### Net Worth

Shows your current net worth as a single number and a chart of how it
has changed over the last twelve months. Net worth is calculated as
total assets minus total liabilities.

### Financial Health Score

A gauge that rates your overall financial health on a scale. The score
takes into account factors like savings rate, debt-to-income ratio,
and budget adherence. Import more transaction history to make the
score more accurate.

### Cash Flow

A bar chart showing monthly income and expenses side by side for the
last twelve months. This helps you see whether you are spending more
or less than you earn over time.

### Spending by Category

A donut chart breaking down your spending into categories for the
current month. Click any category segment to jump to the Transactions
page filtered to that category.

### Upcoming Bills

A list of recurring payments expected in the next 30 days. Each entry
shows the merchant name, expected date, and average amount. Finima
detects these automatically from your transaction history (see
"Recurring Payments" below).

### Budget vs Actual

Progress bars for each budget category you have set up, showing how
much you have spent against your limit. If no budgets are configured,
a link takes you to the Budget page to get started.

---

## Accounts

Accounts represent your real-world bank accounts, credit cards, loans,
and other financial accounts.

### Creating an account

1. Go to the **Accounts** page.
2. Select **+ Add Account**.
3. Fill in the form:
   - **Portfolio** -- which portfolio this account belongs to.
   - **Account Type** -- choose from Checking, Savings, Credit Card,
     Loan, Investment, Retirement, Cash, or Other.
   - **Name** -- a label you will recognize, such as "Chase Checking"
     or "Visa Rewards".
   - **Institution** (optional) -- the bank or financial institution.
   - **Currency** -- the currency this account uses.
   - **Opening Balance** -- the balance at the time you start tracking.
     For credit cards and loans, enter the amount owed as a positive
     number; Finima treats credit card and loan accounts as
     liabilities automatically.
   - **Notes** (optional) -- any extra information for your reference.
4. Select **Create Account**.

### Account types

| Type        | Treated as | Example                           |
| ----------- | ---------- | --------------------------------- |
| Checking    | Asset      | Day-to-day bank account           |
| Savings     | Asset      | Savings or money-market account   |
| Credit Card | Liability  | Visa, Mastercard balance          |
| Loan        | Liability  | Mortgage, auto loan, student loan |
| Investment  | Asset      | Brokerage account                 |
| Retirement  | Asset      | 401(k), IRA                       |
| Cash        | Asset      | Physical cash                     |
| Other       | Asset      | Anything else                     |

### Net worth summary

At the bottom of the Accounts page, a summary row shows your total
assets, total liabilities, and net worth across all accounts in the
active portfolio.

### Viewing account details

Click any account card to open its detail page. From there you can
view the account's transaction history and import new statements.

### Multiple portfolios

If you have more than one portfolio, a dropdown at the top of the
Accounts page lets you switch between them. Most people only need one
portfolio, but you might create a second one to separate personal and
business finances (see "Multi-Portfolio" below).

---

## Importing transactions

Finima imports transactions from bank statement files. It does not
connect to your bank directly -- you download a statement from your
bank's website and upload it to Finima.

### Supported formats

| Format          | Extensions             | Notes                                     |
| --------------- | ---------------------- | ----------------------------------------- |
| CSV / TSV       | `.csv`, `.tsv`         | Most banks offer CSV export               |
| OFX / QFX / QBO | `.ofx`, `.qfx`, `.qbo` | Open Financial Exchange, widely supported |
| QIF             | `.qif`                 | Quicken Interchange Format                |
| Excel           | `.xls`, `.xlsx`        | Spreadsheet format                        |

The maximum file size is 50 MB.

### How to import

1. Go to the account detail page for the target account.
2. Drag a file into the upload area, or click to browse your computer.
3. Finima detects the file format automatically from the extension.
4. After the file uploads, a **column mapping** screen appears.
   For CSV and Excel files, you map the columns in your file to
   Finima's fields: date, description, amount, and optionally
   category. OFX/QFX/QBO and QIF files are parsed automatically and
   skip this step.
5. Confirm the mapping and Finima imports the transactions.

### Deduplication

Finima checks for duplicate transactions during import. If a
transaction with the same date, description, and amount already
exists in the same account, it is skipped. This means you can safely
re-import overlapping date ranges without creating duplicates.

### Automatic categorization

Finima automatically categorizes each imported transaction using
built-in merchant lookup and pattern matching. These rule-based
engines handle the majority of common transactions (groceries, gas,
streaming services, payroll, and so on) without any AI model.

You can change any assigned category manually -- click the category
cell on the Transactions page and select a new value from the
dropdown. Your overrides are preserved on future imports.

### Optional AI categorization

For higher accuracy on ambiguous transactions, you can optionally
enable an AI model. When enabled, transactions that the built-in
engines cannot categorize are sent to the LLM for classification.
See the [Maintainer Guide](maintainer-guide.md#llm-backend-configuration)
for setup instructions.

To verify AI is active, check the **Settings > LLM** tab -- the
connection status should show "Connected". If no AI model is
configured, the status shows "Disabled" and categorization relies
on the built-in engines plus your manual corrections.

---

## Transactions

The Transactions page shows all transactions across all accounts in
the active portfolio.

### Viewing and navigating

Transactions are displayed in a table with columns for date,
description, category, amount, and account. The table is paginated
with 30 transactions per page.

### Sorting

Click any column header to sort by that column. Click again to reverse
the sort order. By default, transactions are sorted by date with the
most recent first.

### Filtering

Use the filter controls above the table to narrow the list:

- **Search** -- type a keyword to search transaction descriptions.
- **Account** -- show only transactions from a specific account.
- **Category** -- show only transactions in a specific category.
- **Date range** -- show transactions within a date range.

### Editing categories

To change the category of a single transaction, click the category
cell in that row and select a new category from the dropdown.

### Bulk editing

To change the category of multiple transactions at once:

1. Select the checkboxes next to the transactions you want to update.
2. Choose a new category from the bulk-action dropdown.
3. Confirm the change.

All selected transactions are updated. Bulk edits mark the category as
user-overridden, which means AI will not re-categorize them on future
imports.

### Exporting

Select the **Export CSV** button to download the current transaction
list as a CSV file. The export respects any active filters.

---

## Budgets

The Budget page helps you set spending limits per category and track
how you are doing each month.

### Setting a budget

There are two ways to create budget entries:

**Manual:** Select **+ New Budget Entry**, type a category name and a
spending limit, then select **Create**.

**Auto-Suggest:** Select **Auto-Suggest Budget**. Finima analyzes your
spending over the past three months and suggests budget limits for each
category based on your averages. Review each suggestion and select
**Apply** to accept it, or dismiss suggestions you do not want.

### Viewing budget progress

The budget table shows one row per category with columns for:

- **Category** -- the spending category.
- **Budget** -- the limit you set.
- **Spent** -- how much you have spent this month.
- **Remaining** -- how much is left (shown in red if over budget).
- **Progress** -- a visual bar showing the percentage used.

A totals row at the bottom summarizes all categories.

### Editing a budget

Select **Edit** next to any category to change the budget limit
inline. Press Enter to save or Escape to cancel.

### Month navigation

Use the **Prev** and **Next** buttons at the top right to look at
budgets for other months. Each month has its own set of budget
entries and spending totals.

---

## Savings Goals

Savings goals help you save toward specific targets. You can access
them from the dedicated **Goals** page or from the bottom of the
**Budget** page.

### Creating a goal

1. Select **+ New Goal**.
2. Enter a **name** (for example, "Emergency Fund" or "Vacation").
3. Enter a **target amount** -- the total you want to save.
4. Optionally set a **target date**.
5. Optionally set a **monthly contribution** -- Finima uses this to
   estimate how many months until you reach your goal.
6. Select **Create Goal**.

### Tracking progress

Each goal is displayed as a card showing:

- Current amount saved and target amount.
- A progress bar with the percentage complete.
- Target date (if set).
- Estimated months to completion based on your monthly contribution.

### Deleting a goal

Select **Delete** on any goal card to remove it.

---

## Recurring Payments

Finima automatically detects recurring payments by analyzing patterns
in your transaction history.

### How detection works

The system looks for transactions with similar descriptions and
amounts that repeat on a regular schedule (weekly, biweekly, monthly,
quarterly, or yearly). Each detected pattern is shown with a
confidence score indicating how certain Finima is about the
recurrence.

### What you see

The Recurring Payments page displays a table with:

- **Name** -- the merchant or payee name.
- **Category** -- the spending category (if categorized).
- **Frequency** -- how often the payment occurs.
- **Amount** -- the average payment amount.
- **Next Expected** -- when the next payment is predicted.
- **Type** -- whether it is income or an expense.
- **Confidence** -- how confident Finima is in the pattern.

Confirmed recurring payments are marked with a "Confirmed" badge.

### Upcoming bills on the dashboard

The Upcoming Bills widget on the dashboard pulls from this same data,
showing recurring payments expected in the next 30 days.

---

## Cash Flows

The Flows page visualizes how money moves between your accounts and
spending categories. It has three tabs.

### Sankey diagram

The Sankey tab shows a flow diagram of where your money comes from
and where it goes. Income sources appear on the left, accounts in the
middle, and spending categories on the right. The width of each flow
is proportional to the amount.

Below the diagram, an **outflow ranking** table lists each destination
account or category sorted by how much money flows to it. The table
shows the monthly amount, percentage of income, and trend (increasing
or decreasing compared to previous months).

### Balance Impact (waterfall chart)

The Balance Impact tab shows a waterfall chart for a selected primary
income account. It visualizes how the opening balance changes through
income and expenses to arrive at the closing balance for the month.

Use the account dropdown to switch between primary income accounts.

### Flow Groups

Flow Groups let you organize related inter-account transfers. For
example, if you regularly transfer money from your checking account
to a savings account, you can create a flow group to track that
pattern.

Each group shows the source and destination accounts, average transfer
amount, frequency, and number of flows detected.

### Month navigation

Use the **Prev** and **Next** buttons to view flows for different
months.

---

## News Feed

The News page aggregates financial news articles from sources
configured on the server (by default, Investopedia and NerdWallet).

### Browsing articles

Articles are displayed as cards showing the source, date, title,
summary, relevance score (1--5 stars), and topic tags. Click any card
to open the full article in a new browser tab.

### Filtering by topic

Use the topic buttons at the top of the page to filter articles:
All, Budgeting, Investing, Taxes, Credit, or Retirement.

### Pagination

Articles are loaded 20 at a time. Use the Previous and Next buttons
at the bottom to page through results.

---

## Settings

The Settings page is organized into four tabs.

### Theme

- **Appearance** -- switch between light, dark, and system (follows
  your operating system) themes.
- **Accent color** -- pick a custom accent color for buttons and
  highlights. A live preview card shows how your choices look.

### Layout

- **Dashboard widgets** -- toggle individual dashboard widgets on or
  off with checkboxes.
- **Reset to Default Layout** -- restore the original dashboard
  widget arrangement.

### General

- **Currency** -- set your display currency (USD, EUR, GBP, CAD, AUD,
  JPY, CHF, or NZD).
- **Date Format** -- choose how dates appear throughout the app
  (MM/DD/YYYY, DD/MM/YYYY, or YYYY-MM-DD).
- **Fiscal Year Start Month** -- set which month your fiscal year
  begins for reporting purposes.
- **Default Chart Type** -- choose between line, bar, and area charts.

### LLM

This tab displays the AI configuration (set on the server) for
reference:

- **Provider** -- the active LLM backend. Possible values:
  - **Ollama** -- HTTP-based inference using an Ollama container.
    Easiest to set up; the container runs via `docker-compose`.
  - **Candle** -- in-process inference via mistral.rs. Lower latency,
    no sidecar container required. Needs the `candle` compile-time
    feature flag (with `metal` or `cuda` for GPU acceleration).
  - **Stub** -- no real LLM. All categorization returns "other" with
    confidence 0.5. Active when neither feature is compiled in, or
    when the configured backend fails to initialize.
- **Model** -- the model name (for example, Gemma 4).
- **Endpoint URL** -- the Ollama server address (shown only when
  the provider is Ollama).
- **Connection Status** -- whether the backend can reach the LLM
  service (Connected, Disconnected, or Checking).

LLM settings are configured in `config/llm.yaml` on the server
(or via `APP__LLM__*` environment variables in `.env`), not through
the UI. See the
[Maintainer Guide](maintainer-guide.md#llm-backend-configuration) for
setup instructions and the Troubleshooting guide if the status shows
Disconnected.

### Saving settings

After making changes on any tab, select **Save Preferences** at the
bottom of the page to persist your settings.

---

## Multi-Portfolio

A portfolio is a top-level container that groups accounts together.
Most people need only one, but you might create additional portfolios
to keep finances separate -- for example:

- Personal vs. business finances.
- Tracking finances for different family members.
- Separating a side project's money from your main accounts.

### How to use multiple portfolios

1. During onboarding you create your first portfolio.
2. To create another, go to the **Accounts** page and use the
   portfolio controls.
3. A portfolio selector dropdown appears at the top of the Accounts
   page whenever you have more than one portfolio.
4. Switching portfolios changes the accounts, transactions, budgets,
   and dashboard data shown throughout the app.

Each portfolio has its own set of accounts, transactions, budgets,
and savings goals. Data does not cross between portfolios.

---

## Keyboard and accessibility notes

- All interactive elements are reachable by keyboard.
- Modals trap focus so you can tab through form fields without
  accidentally leaving the dialog.
- Screen readers can navigate tables, progress bars, and charts using
  standard ARIA landmarks and labels.

---

## Getting help

- [Quick Start](quick-start.md) -- initial setup instructions.
- [Troubleshooting](troubleshooting.md) -- solutions to common
  problems.
