# Screenshots

A visual tour of Finima's UI. Click any thumbnail to jump to the full-size image and a short description of what that screen does.

## Gallery

<table>
  <tr>
    <td align="center" width="33%">
      <a href="#sign-in"><img src="../../images/01a-sign-in.png" alt="Sign in" width="280"/></a><br/>
      <sub><b>Sign in</b></sub>
    </td>
    <td align="center" width="33%">
      <a href="#sign-in-with-token"><img src="../../images/01b-sign-in-with-token.png" alt="Sign in with token" width="280"/></a><br/>
      <sub><b>Sign in with token</b></sub>
    </td>
    <td align="center" width="33%">
      <a href="#dashboard"><img src="../../images/02-dashboard.png" alt="Dashboard" width="280"/></a><br/>
      <sub><b>Dashboard</b></sub>
    </td>
  </tr>
  <tr>
    <td align="center">
      <a href="#portfolios"><img src="../../images/03-portfolios.png" alt="Portfolios" width="280"/></a><br/>
      <sub><b>Portfolios</b></sub>
    </td>
    <td align="center">
      <a href="#accounts"><img src="../../images/04-accounts.png" alt="Accounts" width="280"/></a><br/>
      <sub><b>Accounts</b></sub>
    </td>
    <td align="center">
      <a href="#transactions"><img src="../../images/05-transactions.png" alt="Transactions" width="280"/></a><br/>
      <sub><b>Transactions</b></sub>
    </td>
  </tr>
  <tr>
    <td align="center">
      <a href="#payee-rules"><img src="../../images/06-payee-rules.png" alt="Payee rules" width="280"/></a><br/>
      <sub><b>Payee rules</b></sub>
    </td>
    <td align="center">
      <a href="#recurring"><img src="../../images/07-recurring.png" alt="Recurring" width="280"/></a><br/>
      <sub><b>Recurring</b></sub>
    </td>
    <td align="center">
      <a href="#money-flow"><img src="../../images/08-money-flow.png" alt="Money flow" width="280"/></a><br/>
      <sub><b>Money flow</b></sub>
    </td>
  </tr>
  <tr>
    <td align="center">
      <a href="#budget"><img src="../../images/09-budget.png" alt="Budget" width="280"/></a><br/>
      <sub><b>Budget</b></sub>
    </td>
    <td align="center">
      <a href="#goals"><img src="../../images/10-goals.png" alt="Goals" width="280"/></a><br/>
      <sub><b>Goals</b></sub>
    </td>
    <td align="center">
      <a href="#news"><img src="../../images/11-news.png" alt="News" width="280"/></a><br/>
      <sub><b>News</b></sub>
    </td>
  </tr>
  <tr>
    <td align="center">
      <a href="#settings"><img src="../../images/12-settings.png" alt="Settings" width="280"/></a><br/>
      <sub><b>Settings</b></sub>
    </td>
    <td></td>
    <td></td>
  </tr>
</table>

---

## Sign in

[![Sign in](../../images/01a-sign-in.png)](../../images/01a-sign-in.png)

Passwordless magic-link sign-in. Enter your email and Finima sends a one-time link -- no passwords to remember or leak. The same screen handles first-time registration; the first account to register becomes the instance owner.

## Sign in with token

[![Sign in with token](../../images/01b-sign-in-with-token.png)](../../images/01b-sign-in-with-token.png)

The second step of the magic-link flow. Finima verifies the token from the email, establishes a session, and redirects to the dashboard. Tokens are single-use and short-lived.

## Dashboard

[![Dashboard](../../images/02-dashboard.png)](../../images/02-dashboard.png)

The at-a-glance home screen. Shows the financial health score, net worth, cash flow trend, budget vs. actual tile, top categories, and recent activity. Everything tiles are scoped to the currently selected portfolio.

## Portfolios

[![Portfolios](../../images/03-portfolios.png)](../../images/03-portfolios.png)

Portfolios group accounts into separate financial contexts (personal, business, household, etc.). Each portfolio has its own transactions, budgets, and goals; deleting a portfolio cascades to its owned data.

## Accounts

[![Accounts](../../images/04-accounts.png)](../../images/04-accounts.png)

Manage bank, credit, loan, and investment accounts. Import transactions from CSV, OFX, QIF, or XLSX with a column-mapping UI. Balances and activity are recomputed as data lands.

## Transactions

[![Transactions](../../images/05-transactions.png)](../../images/05-transactions.png)

The full ledger view: filter by account, category, date range, or free-text search; bulk-edit categories; split transactions; and review AI-suggested categorizations before accepting them.

## Payee rules

[![Payee rules](../../images/06-payee-rules.png)](../../images/06-payee-rules.png)

Pattern-based rules that auto-categorize transactions by payee. Rules run on import and can be previewed against existing data before saving, so you can see exactly what will change.

## Recurring

[![Recurring](../../images/07-recurring.png)](../../images/07-recurring.png)

Finima detects subscriptions and regular payments from your transaction history and surfaces them here. Useful for spotting forgotten subscriptions and forecasting upcoming cash outflows.

## Money flow

[![Money flow](../../images/08-money-flow.png)](../../images/08-money-flow.png)

A Sankey diagram visualizing inter-account movement over a time range: where income arrives, how it splits across spending categories, and what lands in savings or investment accounts.

## Budget

[![Budget](../../images/09-budget.png)](../../images/09-budget.png)

Set monthly budgets by category and track progress in real time. The view highlights over-budget categories and shows pace-of-spend against the month's elapsed days.

## Goals

[![Goals](../../images/10-goals.png)](../../images/10-goals.png)

Define savings goals with target amounts and dates. Finima tracks contributions automatically from linked accounts and projects completion based on current pace.

## News

[![News](../../images/11-news.png)](../../images/11-news.png)

Aggregated financial news feed with on-device LLM summaries. Articles are fetched from configured sources; summaries are generated locally via Ollama so nothing about your reading habits leaves the machine.

## Settings

[![Settings](../../images/12-settings.png)](../../images/12-settings.png)

Theme (light/dark), currency, date format, LLM model selection, notification preferences, and account/session management. Instance-level settings are separated from per-user preferences.
