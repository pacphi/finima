# Glossary

Definitions for terms you will encounter while using Finima. Terms are grouped by type: financial terms, file format terms, and app-specific terms.

---

## Financial terms

**Asset**
Something you own that has value. Examples include money in a checking account, a savings account balance, investments, or retirement funds. In Finima, accounts of type Checking, Savings, Investment, Retirement, and Cash are treated as assets.

**Budget**
A planned spending limit for a category over a period of time — usually one month. For example, you might set a budget of $500 for groceries. Finima compares your actual spending to your budget and shows the difference on the dashboard. Finima can suggest starting budget amounts based on your recent transaction history.

**Cash flow**
The difference between money coming in (income) and money going out (expenses) over a period of time. Positive cash flow means you spent less than you earned. Negative cash flow means you spent more than you earned. The Cash Flow widget on the Finima dashboard shows this comparison month by month.

**Category (as used in Finima)**
A label that describes what a transaction is for — for example, Groceries, Rent, Streaming, or Payroll. Finima automatically suggests a category for each imported transaction and lets you override the suggestion. Categories are used to group spending in charts and in budget tracking.

**Debt-to-income ratio**
A percentage that compares how much you owe each month (debt payments) to how much you earn. For example, if you earn $5,000 a month and your debt payments total $1,500, your debt-to-income ratio is 30%. Lenders use this number to assess risk; Finima uses it as one input to the Financial Health Score.

**Fiscal year**
A 12-month period used for financial record-keeping. It does not have to match the calendar year. Many businesses use a fiscal year that starts on a date other than January 1. For personal finances, the fiscal year usually matches the calendar year (January–December).

**Financial health score**
A single number (shown as a gauge on the Finima dashboard) that summarizes how well your overall finances are doing. It is calculated from several factors including your savings rate, debt-to-income ratio, and how closely you are sticking to your budgets. The more transaction history you import, the more accurate the score becomes.

**Liability**
Something you owe to someone else. Examples include a credit card balance, a mortgage, a car loan, or a student loan. In Finima, accounts of type Credit Card and Loan are treated as liabilities. Net worth is calculated by subtracting total liabilities from total assets.

**Net worth**
The total value of everything you own (assets) minus the total of everything you owe (liabilities). A positive net worth means your assets are worth more than your debts. Net worth is the most common single-number summary of financial health. Finima calculates and charts your net worth on the dashboard.

**Recurring payment**
A payment that happens on a regular schedule — weekly, monthly, or yearly — such as a rent payment, a streaming subscription, or a gym membership. Finima scans your transaction history and automatically identifies these patterns, then shows upcoming recurring payments on the dashboard so you know what to expect.

**Savings goal**
A target amount you want to save for a specific purpose, such as an emergency fund, a vacation, or a down payment on a car. You set the target amount and an optional target date, and Finima tracks your progress toward it.

---

## File format terms

**CSV (Comma-Separated Values) — `.csv`**
A plain-text file where each line is a row of data and each value within a row is separated by a comma. Because CSV is so simple and widely supported, almost every bank offers CSV export. Finima accepts `.csv` files and shows a column-mapping screen so you can match your bank's column names to Finima's fields.

**Excel spreadsheet — `.xls`, `.xlsx`**
A spreadsheet file created by Microsoft Excel. `.xls` is the older format (Excel 97–2003); `.xlsx` is the modern format. Some banks offer spreadsheet downloads instead of (or in addition to) CSV. Finima can read both formats and handles the column mapping the same way it does for CSV files.

**OFX / QFX / QBO (Open Financial Exchange) — `.ofx`, `.qfx`, `.qbo`**
A standardized file format designed specifically for financial data exchange. OFX is the generic open standard; QFX is Intuit's variant used by Quicken; QBO is Intuit's variant used by QuickBooks. Because these formats include structured field names, Finima can parse them automatically without a column-mapping step.

**QIF (Quicken Interchange Format) — `.qif`**
An older file format originally created for Quicken personal finance software. Many banks and financial applications still export QIF for compatibility. Like OFX, QIF is self-describing, so Finima imports it without a column-mapping step.

**TSV (Tab-Separated Values) — `.tsv`**
A plain-text file similar to CSV, but with tab characters separating values instead of commas. Some banks and brokerages export TSV files. Finima handles TSV and CSV the same way.

---

## App terms

**Account**
In Finima, an account represents one real-world financial account — a bank checking account, a savings account, a credit card, a loan, or an investment account. Each account belongs to a portfolio and holds its own transaction history. You import transactions by uploading a statement file to an account.

**Magic link**
A sign-in method that does not require a password. Instead of typing a password, you enter your email address and Finima sends you a link. Clicking that link signs you in. The link expires after a short time and can only be used once. In the default local setup, because no email service is configured, the link is printed directly in the terminal rather than sent by email — see [Step 5 of the getting-started guide](getting-started.md#step-5-sign-in-for-the-first-time).

**Portfolio**
A named group of accounts. Most users have a single portfolio (for example, "Personal") that contains all of their accounts. If you want to keep separate sets of books — say, personal finances and a side business — you can create a second portfolio. Switching between portfolios changes which accounts and summaries are shown throughout the app.

**Sankey diagram**
A type of flow chart where the width of each band is proportional to the quantity it represents. In Finima, the Sankey diagram shows how money flows from income sources through spending categories, making it easy to see at a glance where the largest portions of your income go. You can find it in the Cash Flow section of the app.

**Transaction**
A single financial event recorded in an account — a purchase, a payment, a deposit, or a transfer. Each transaction has a date, a description (usually the merchant or payee name), an amount, and a category. Transactions are imported from bank statement files and form the foundation for all of Finima's charts, budgets, and scores.
