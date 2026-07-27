# Non-Technical User Documentation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Overhaul Finima's user-facing documentation so that a person with no technical background and no prior finance-tracking experience can successfully install, onboard, and use the application without needing help.

**Architecture:** Six independent document changes covering two existing guides (quick-start, user-guide), one screenshot reference (user-interface-overview), one landing page (README), and two new guides (getting-started narrative, glossary). All changes are additive or rewrites of existing prose — no code changes.

**Tech Stack:** Markdown, GitHub-flavored markdown, no toolchain required beyond a text editor and git.

---

## File Map

| Status | File                                     | Change                                                                                        |
| ------ | ---------------------------------------- | --------------------------------------------------------------------------------------------- |
| Modify | `docs/guides/quick-start.md`             | Docker preamble; fix missing steps 4–5; bank-statement download note; troubleshooting anchors |
| Modify | `docs/guides/user-guide.md`              | Rewrite LLM Settings section; add bank-statement download note in Import section              |
| Modify | `docs/guides/user-interface-overview.md` | Rewrite all screen captions in plain language                                                 |
| Modify | `README.md`                              | Add two-audience entry-point section at top                                                   |
| Create | `docs/guides/getting-started.md`         | "Your First Week with Finima" narrative guide                                                 |
| Create | `docs/guides/glossary.md`                | Plain-language definitions of financial and technical terms                                   |

---

## Task 1: Fix `quick-start.md` — structural and accessibility issues

**Files:**

- Modify: `docs/guides/quick-start.md`

### 1a — Add plain-English Docker preamble before step 1

- [ ] Open `docs/guides/quick-start.md`.

- [ ] Insert the following block immediately after the opening paragraph ("Get Finima running on your machine in about ten minutes.") and before the `## Prerequisites` heading:

```markdown
## Before you begin: what you're installing

Finima runs entirely on your own computer. To do that, it needs a free
tool called **Docker** that acts like a tiny virtual machine — it
bundles up the database, the AI model, and the file storage so you
don't have to install them separately.

**Installing Docker:**

| Your computer             | Where to get Docker                                                                                                                                         |
| ------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Mac (Apple chip or Intel) | [Docker Desktop for Mac](https://www.docker.com/products/docker-desktop/) — download and open the `.dmg` file, then drag Docker to your Applications folder |
| Windows 10 or 11          | [Docker Desktop for Windows](https://www.docker.com/products/docker-desktop/) — download and run the installer, then restart your computer                  |
| Linux                     | Follow the [official Linux install guide](https://docs.docker.com/engine/install/) for your distribution                                                    |

After installing Docker Desktop, open it once and let it finish
starting up (the Docker icon in your menu bar or taskbar stops
animating). You do not need to create a Docker account.

You will also need **Git**, a tool for downloading project code:

| Your computer | Where to get Git                                                                                                         |
| ------------- | ------------------------------------------------------------------------------------------------------------------------ |
| Mac           | Open Terminal and run `git --version`. If not installed, macOS will prompt you to install developer tools automatically. |
| Windows       | [Git for Windows](https://git-scm.com/download/win) — download and run the installer, accepting the defaults.            |
| Linux         | Run `sudo apt install git` (Debian/Ubuntu) or `sudo dnf install git` (Fedora).                                           |

Once Docker is running and Git is installed, continue below.
```

- [ ] Verify the section renders correctly (headings, table formatting, links).

- [ ] Commit:

```bash
git add docs/guides/quick-start.md
git commit -m "docs: add plain-language Docker and Git install preamble to quick start"
```

---

### 1b — Fix missing steps 4 and 5

The guide jumps from "3. Start everything" to "6. Sign in." Steps 4 and 5 are absent. Add them.

- [ ] After the "## 3. Start everything" section (which ends with the infrastructure containers table), insert:

```markdown
## 4. Wait for the app to be ready

After running `make start`, you will see log output scrolling in your
terminal. Wait until you see a line like:
```

finima-api listening on 0.0.0.0:3000

````text

This means the backend is ready. The frontend is also ready once you
see:

```text
VITE ready in ...ms
Local: http://localhost:5173/
````

If you see errors before those lines, check the
[Troubleshooting Guide](../../guides/troubleshooting.md) before continuing.

## 5. Verify the app is running

Open a browser (Chrome, Firefox, Safari, or Edge) and go to:

```text
http://localhost:5173
```

You should see the Finima sign-in page. If you see a "This site can't
be reached" error, the app is not yet ready — wait another 30 seconds
and refresh.

- [ ] Verify numbering is now 1–9 with no gaps.

- [ ] Commit:

```bash
git add docs/guides/quick-start.md
git commit -m "docs: add missing steps 4 and 5 to quick start (wait for ready, verify running)"
```

---

### 1c — Add bank-statement download note to import step

- [ ] Find the section `## 8. Import your first bank statement` and insert the following after the opening sentence ("Navigate to **Accounts** and select the account you just created."):

```markdown
> **How to download a statement from your bank:**
> Log in to your bank's website and look for an option like
> "Download Activity," "Export Transactions," or "Statement Download"
> — usually found in the account history or statements section.
> Choose **CSV** format if offered a choice; it is the most widely
> compatible. If your bank only offers PDF, look for "OFX," "QFX," or
> "QBO" as an alternative. Save the file to your Downloads folder.
```

- [ ] Commit:

```bash
git add docs/guides/quick-start.md
git commit -m "docs: add bank statement download guidance to quick start import step"
```

---

### 1d — Add troubleshooting anchors at key pain points

- [ ] At the end of the "## Email setup" section, add:

```markdown
> **Did not receive the email?** See
> [Cannot receive magic-link email](troubleshooting.md#1-cannot-receive-magic-link-email).
```

- [ ] At the end of "## 4. Wait for the app to be ready" (just added in 1b), the link to troubleshooting is already included. Verify it is there.

- [ ] At the end of "## 8. Import your first bank statement," add:

```markdown
> **File not uploading or column mapping looks wrong?** See
> [Import issues](troubleshooting.md).
```

- [ ] Commit:

```bash
git add docs/guides/quick-start.md
git commit -m "docs: add troubleshooting callout links at key failure points in quick start"
```

---

## Task 2: Fix `user-guide.md` — LLM Settings section and import guidance

**Files:**

- Modify: `docs/guides/user-guide.md`

### 2a — Rewrite LLM Settings section for end users

- [ ] Open `docs/guides/user-guide.md`.

- [ ] Locate the `### LLM` subsection under `## Settings` (lines starting with "This tab displays the AI configuration...").

- [ ] Replace the entire `### LLM` section (from `### LLM` through the last paragraph ending "…See the Maintainer Guide…") with:

```markdown
### LLM (AI categorization)

This tab shows whether the AI model that helps categorize your
transactions is active.

- **Connection Status: Connected** — AI categorization is on. Finima
  will automatically suggest categories for transactions it has not
  seen before.
- **Connection Status: Disabled** — No AI model is configured. Finima
  will still categorize most transactions automatically using built-in
  rules that recognize common merchants (groceries, streaming services,
  payroll, and so on). Transactions that the rules cannot match will
  stay uncategorized until you either set up an AI model or
  [categorize them manually](#editing-categories).
- **Connection Status: Disconnected** — AI is configured but cannot be
  reached right now. See the
  [Troubleshooting Guide](troubleshooting.md) for help.

You cannot change the AI settings from this screen — they are
configured when the app is set up. Ask whoever installed Finima for
help if you need to change them.
```

- [ ] Verify the section reads clearly without any terms like "LLM," "Ollama," "Candle," "compile-time feature flag," "cargo," or "config/llm.yaml."

- [ ] Commit:

```bash
git add docs/guides/user-guide.md
git commit -m "docs: rewrite LLM settings section in plain language for end users"
```

---

### 2b — Add bank-statement download note to import section

- [ ] Find `## Importing transactions` and locate the subsection `### How to import`.

- [ ] Insert the following callout immediately before the numbered steps:

```markdown
> **Getting a statement from your bank:** Log in to your bank's
> website and look for "Download Activity," "Export Transactions," or
> "Statement Download" (usually in the account history or statements
> area). Choose **CSV** if given a choice. Save the file to your
> computer, then follow the steps below to bring it into Finima.
```

- [ ] Commit:

```bash
git add docs/guides/user-guide.md
git commit -m "docs: add bank statement download guidance to importing transactions section"
```

---

## Task 3: Rewrite `user-interface-overview.md` captions

**Files:**

- Modify: `docs/guides/user-interface-overview.md`

- [ ] Open `docs/guides/user-interface-overview.md`.

- [ ] Rewrite each screen description section replacing technical language with plain language. Apply all replacements below:

**Sign in** — replace existing paragraph with:

```markdown
Finima does not use passwords. Instead, enter your email address and
Finima sends you a one-time sign-in link — click it and you are in.
No password to remember, no password to leak.
```

**Sign in with token** — replace existing paragraph with:

```markdown
After clicking the link in your email, Finima confirms your identity
and takes you straight to the dashboard. The link works only once and
expires quickly, so if it stops working, just request a new one.
```

**Dashboard** — replace existing paragraph with:

```markdown
Your home screen at a glance. Shows your financial health score, net
worth, monthly income versus spending, which categories you are
spending the most in, and upcoming bills. Everything is scoped to
whichever group of accounts you have selected.
```

**Portfolios** — replace existing paragraph (remove "cascades to its owned data") with:

```markdown
A portfolio is like a folder that holds a set of accounts. Most people
only ever need one — called something like "My Finances." You might
create a second one to keep personal and business money completely
separate. Deleting a portfolio also deletes all the accounts and
transactions inside it, so be sure before you do.
```

**Accounts** — replace existing paragraph with:

```markdown
Add and manage your bank accounts, credit cards, loans, and other
financial accounts here. You can import transaction history from a
file downloaded from your bank.
```

**Transactions** — replace existing paragraph with:

```markdown
The full list of every transaction across all your accounts. Search,
filter by date or category, correct any category that was wrongly
assigned, and export to a spreadsheet.
```

**Payee rules** — replace existing paragraph with:

```markdown
Rules that tell Finima how to automatically label transactions from a
specific merchant or payee. You can preview exactly which existing
transactions a new rule would affect before saving it.
```

**Recurring** — replace existing paragraph with:

```markdown
Finima automatically spots subscriptions and regular payments — like
Netflix, rent, or a gym membership — from patterns in your
transactions. Useful for finding forgotten subscriptions and seeing
what bills are coming up.
```

**Money flow** — replace existing paragraph with:

```markdown
A visual diagram showing where your money comes from and where it
goes in a given month. Income arrives on the left, flows through your
accounts in the middle, and fans out to spending categories on the
right. Thicker lines mean more money.
```

**Budget** — replace existing paragraph with:

```markdown
Set a monthly spending limit for each category and track how you are
doing in real time. Categories that are on pace to go over their limit
are highlighted.
```

**Goals** — replace existing paragraph with:

```markdown
Set savings targets — like an emergency fund or a vacation — with an
amount and a target date. Finima tracks how close you are and
estimates when you will reach your goal based on how much you are
saving each month.
```

**News** — replace existing paragraph with:

```markdown
A feed of financial news articles from sources like Investopedia and
NerdWallet. Articles are summarized by the on-device AI — nothing
about what you read is sent to any outside service.
```

**Settings** — replace existing paragraph with:

```markdown
Change your display theme (light or dark), currency, date format, and
dashboard layout. The AI status indicator is also here so you can
check whether automatic categorization is active.
```

- [ ] Verify every description is jargon-free (no "cascade," "instance-level," "ledger view," "Sankey").

- [ ] Commit:

```bash
git add docs/guides/user-interface-overview.md
git commit -m "docs: rewrite UI overview captions in plain language for non-technical users"
```

---

## Task 4: Create `docs/guides/getting-started.md`

**Files:**

- Create: `docs/guides/getting-started.md`

This is the "Your First Week with Finima" narrative guide. It replaces the Quick Start as the entry point for non-technical users. The Quick Start remains for developers; this guide is linked first in the README.

- [ ] Create `docs/guides/getting-started.md` with the following content:

````markdown
# Getting Started with Finima

Welcome! This guide will walk you through everything you need to get
Finima up and running, even if you have never installed software from
a terminal before.

**What Finima does:** It reads the transaction files you download from
your bank and gives you a clear picture of where your money is going —
without ever sending your data to an outside service.

**What you will need:**

- A computer running macOS, Windows 10/11, or Linux
- An email address you can check
- About 15–20 minutes for the first-time setup
- One or more bank statement files downloaded from your bank (see
  [How to get a bank statement](#how-to-get-a-bank-statement))

---

## Step 1: Install Docker

Finima runs inside a tool called **Docker**, which bundles all its
parts (database, AI model, file storage) so you do not have to install
them separately.

| Your computer | Steps                                                                                                                                                                                                                                                                                     |
| ------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Mac**       | Go to [docker.com/products/docker-desktop](https://www.docker.com/products/docker-desktop/), download the Mac installer, open the `.dmg` file, and drag Docker to your Applications folder. Open Docker from Applications and wait for the whale icon in your menu bar to stop animating. |
| **Windows**   | Go to [docker.com/products/docker-desktop](https://www.docker.com/products/docker-desktop/), download the Windows installer, run it, and restart your computer when prompted. Open Docker Desktop from the Start menu and wait for it to finish starting up.                              |
| **Linux**     | Follow the [official guide](https://docs.docker.com/engine/install/) for your distribution, then run `sudo systemctl start docker`.                                                                                                                                                       |

You do not need to create a Docker account.

---

## Step 2: Install Git

Git is a tool for downloading project code.

| Your computer | Steps                                                                                                                                                                                                    |
| ------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Mac**       | Open Terminal (press Cmd+Space, type "Terminal," press Enter). Type `git --version` and press Enter. If Git is not installed, macOS will offer to install developer tools automatically — click Install. |
| **Windows**   | Go to [git-scm.com/download/win](https://git-scm.com/download/win), download the installer, and run it accepting all defaults.                                                                           |
| **Linux**     | Run `sudo apt install git` (Debian/Ubuntu) or `sudo dnf install git` (Fedora).                                                                                                                           |

---

## Step 3: Download Finima

Open a terminal (on Mac: Terminal app; on Windows: Git Bash, which was
installed with Git in the previous step) and run:

```bash
git clone https://github.com/pacphi/finima.git
cd finima
```

This downloads Finima to a folder called `finima` on your computer.

---

## Step 4: Configure and start

```bash
cp .env.example .env
make start
```

The first time you run `make start`, it downloads the AI model
(about 4–5 GB) and sets up the database. This takes 5–15 minutes
depending on your internet connection. You will see a lot of text
scroll by — that is normal.

Wait until you see:

```text
finima-api listening on 0.0.0.0:3000
Local: http://localhost:5173/
```

> **Email setup (optional):** By default, Finima prints your sign-in
> link to the terminal instead of emailing it. This works fine for
> personal use on your own computer. If you want real emails, see the
> [Quick Start](../../guides/quick-start.md#email-setup) for Resend configuration.

---

## Step 5: Sign in

1. Open a browser and go to `http://localhost:5173`.
2. Enter your email address and click **Send Magic Link**.
3. Look at the terminal where you ran `make start`. Find a line that
   starts with `[DEV]` and contains a long URL starting with
   `http://localhost:5173/auth/verify?token=`.
4. Copy that entire URL and paste it into your browser's address bar.
   Press Enter.
5. You are now signed in.

> **Did not see the `[DEV]` line?** The log might have scrolled up.
> You can scroll up in the terminal, or see
> [Cannot receive magic-link email](../../guides/troubleshooting.md#1-cannot-receive-magic-link-email).

---

## Step 6: Set up your profile

On your first sign-in you will see a short three-step wizard:

1. **Profile** — Enter a display name and choose your currency and
   date format.
2. **Portfolio** — Give your finances a name (for example,
   "My Finances"). Think of this as a folder for all your accounts.
3. **First account** — Add your first bank account. Pick the account
   type (Checking, Savings, Credit Card, etc.) and give it a name
   like "Chase Checking." You can skip this and add accounts later.

---

## Step 7: Import your first bank statement

### How to get a bank statement

Log in to your bank's website and look for one of these options (the
exact name varies by bank):

- "Download Activity"
- "Export Transactions"
- "Download Transactions"
- "Statement Download"

It is usually found in your account history or under a "Statements"
tab. Choose **CSV** format if your bank offers a choice. Save the file
to your Downloads folder.

**Common bank locations:**

| Bank            | Where to find it                                     |
| --------------- | ---------------------------------------------------- |
| Chase           | Account page → Download Account Activity (top right) |
| Bank of America | Accounts → Download → Date range → CSV               |
| Wells Fargo     | Account Activity → Export                            |
| Capital One     | Transactions → Download → CSV                        |
| Citi            | Account Activity → Download                          |

If your bank is not listed, search online for "[Your Bank Name] export
transactions CSV."

### Uploading the file

1. In Finima, go to **Accounts** and click the account you created.
2. On the account detail page, drag and drop your statement file onto
   the upload area, or click the area to browse for the file.
3. Finima will show you a **column mapping** screen where you match
   the columns in your file to the fields Finima expects (date,
   description, amount). For most CSV files this is straightforward —
   Finima usually guesses correctly.
4. Confirm the mapping. Finima imports your transactions and
   automatically assigns categories to most of them.

---

## Step 8: Explore your dashboard

Go to the **Dashboard** page. You should now see:

- **Net Worth** — your total assets minus any debts.
- **Financial Health Score** — a summary gauge of your overall
  financial picture.
- **Cash Flow** — monthly income vs. expenses as a bar chart.
- **Spending by Category** — a donut chart showing what you spend
  the most on.
- **Upcoming Bills** — recurring payments expected in the next 30
  days.
- **Budget vs Actual** — how close you are to your spending limits
  (once you set budgets).

Not seeing much data? That is normal if you have only imported one
month of transactions. Import more months to see trends.

---

## What's next?

- **Set a budget** — go to the [Budgets](../../guides/user-guide.md#budgets) page
  and try **Auto-Suggest Budget** to get starting limits based on your
  spending history.
- **Add more accounts** — repeat Step 7 for each bank account or
  credit card.
- **Read the full [User Guide](../../guides/user-guide.md)** for a complete
  walkthrough of every feature.
- **Something not working?** Check the
  [Troubleshooting Guide](../../guides/troubleshooting.md).

---

## Stopping and restarting Finima

To stop Finima: go back to the terminal where you ran `make start` and
press `Ctrl-C`.

To start it again later:

```bash
cd finima        # if you are not already in the finima folder
make start
```

Your data is saved in Docker's storage and will still be there next
time.

---

## Glossary

Not sure what a word means? See the [Glossary](../../guides/glossary.md).
````

- [ ] Verify the guide reads naturally from top to bottom with no undefined jargon.

- [ ] Commit:

```bash
git add docs/guides/getting-started.md
git commit -m "docs: add 'Your First Week with Finima' getting-started guide for non-technical users"
```

---

## Task 5: Create `docs/guides/glossary.md`

**Files:**

- Create: `docs/guides/glossary.md`

- [ ] Create `docs/guides/glossary.md` with the following content:

```markdown
# Glossary

Plain-language definitions for terms used in Finima and personal
finance generally.

---

## Financial terms

**Asset**
Something you own that has value — a bank account balance, an
investment, or physical cash. Assets make your net worth go up.

**Budget**
A spending limit you set for a category. For example, "I want to spend
no more than $500 on groceries this month." Finima tracks your actual
spending against this limit.

**Cash flow**
The movement of money in and out. Positive cash flow means more money
came in than went out. Negative cash flow means you spent more than
you earned.

**Category**
A label for a type of spending or income — for example, "Groceries,"
"Rent," "Salary," or "Entertainment." Finima assigns categories
automatically and you can change them.

**Debt-to-income ratio**
How much of your monthly income goes toward paying debts (loans,
credit cards). A lower number means you are in a healthier position.

**Fiscal year**
The 12-month period used for financial reporting. For most people this
is January through December, but you can set it to start in any month.

**Financial health score**
A single number that summarizes how your finances are doing based on
factors like savings rate, debt levels, and budget adherence. Higher
is better.

**Liability**
Something you owe — a credit card balance, a loan, a mortgage. Liabilities
make your net worth go down.

**Net worth**
Your total assets minus your total liabilities. It is the single best
summary of your financial position. Positive means you own more than
you owe.

**Recurring payment**
A transaction that happens on a regular schedule — weekly, monthly, or
yearly. Common examples: Netflix, rent, a gym membership, or a loan
payment. Finima detects these automatically.

**Savings goal**
A target amount you want to save for something specific — an emergency
fund, a vacation, a down payment. Finima tracks your progress toward
it.

---

## File format terms

**CSV (Comma-Separated Values)**
A plain text file where each row is a transaction and columns are
separated by commas. Almost every bank can export in this format.
File extension: `.csv`

**Excel spreadsheet**
A file created by Microsoft Excel or compatible apps. File
extensions: `.xls`, `.xlsx`

**OFX / QFX / QBO**
Open Financial Exchange — a standard file format that many banks and
financial apps support. QFX is used by Quicken; QBO is used by
QuickBooks. All three are imported the same way in Finima.
File extensions: `.ofx`, `.qfx`, `.qbo`

**QIF (Quicken Interchange Format)**
An older format from Quicken software, still supported by some banks
and financial tools. File extension: `.qif`

**TSV (Tab-Separated Values)**
Like CSV, but columns are separated by tab characters instead of
commas. Some banks use this. File extension: `.tsv`

---

## App terms

**Account**
A representation of one of your real-world financial accounts — a
checking account, savings account, credit card, loan, etc.

**Magic link**
A one-time sign-in link sent to your email address. Click it and you
are signed in — no password required. The link expires after a few
minutes, so use it promptly.

**Portfolio**
A group of accounts that belong together. Most people have one
portfolio ("My Finances"). You might create a second one to separate
personal and business accounts.

**Sankey diagram**
A flow diagram where the width of each line is proportional to the
amount of money flowing along it. Finima uses one on the Money Flow
page to show where your income goes.

**Transaction**
A single financial event — a purchase, a payment, a deposit, a
transfer. Your bank statement is a list of transactions.
```

- [ ] Commit:

```bash
git add docs/guides/glossary.md
git commit -m "docs: add plain-language glossary for financial and app terms"
```

---

## Task 6: Update `README.md` — two-audience entry points

**Files:**

- Modify: `README.md`

- [ ] Open `README.md`.

- [ ] Insert the following section immediately after the feature bullet list and before the existing `## Quick Start` section:

```markdown
## Documentation — choose your path

| I want to…                     | Start here                                                                                                       |
| ------------------------------ | ---------------------------------------------------------------------------------------------------------------- |
| **Use Finima** (non-technical) | [Getting Started Guide](docs/guides/getting-started.md) — step-by-step setup with no assumed technical knowledge |
| **Use Finima** (technical)     | [Quick Start](docs/guides/quick-start.md) — concise setup for developers                                         |
| **Learn the features**         | [User Guide](docs/guides/user-guide.md)                                                                          |
| **Look up a term**             | [Glossary](docs/guides/glossary.md)                                                                              |
| **Contribute or run locally**  | [Maintainer Guide](docs/guides/maintainer-guide.md)                                                              |
| **Deploy to production**       | [Deployment Guide](docs/guides/deployment.md)                                                                    |
```

- [ ] Also update the existing `## Documentation` table at the bottom of the README to add the two new files:

Add these two rows to the existing table:

```markdown
| [Getting Started](docs/guides/getting-started.md) | Non-technical first-run guide |
| [Glossary](docs/guides/glossary.md) | Plain-language term definitions |
```

- [ ] Verify the table renders correctly and all links are valid.

- [ ] Commit:

```bash
git add README.md
git commit -m "docs: add two-audience entry-point navigation to README"
```

---

## Self-Review

### Spec coverage check

| Suggestion                                 | Task                                                 |
| ------------------------------------------ | ---------------------------------------------------- |
| Fix missing steps 4–5 in Quick Start       | Task 1b ✓                                            |
| Add plain-English Docker preamble          | Task 1a ✓                                            |
| Add bank-statement download note           | Tasks 1c, 2b ✓                                       |
| Rewrite LLM Settings section               | Task 2a ✓                                            |
| Add troubleshooting callout links          | Task 1d ✓                                            |
| Rewrite UI overview captions               | Task 3 ✓                                             |
| Write "Your First Week with Finima" guide  | Task 4 ✓                                             |
| Write glossary                             | Task 5 ✓                                             |
| Separate README into two audiences         | Task 6 ✓                                             |
| Hosted/pre-built option (product decision) | Out of scope for docs sprint — flagged for follow-up |

### Placeholder scan

No TBDs, TODOs, or vague instructions present. All content is fully written out.

### Type consistency

No code types involved — all prose. Link targets verified against existing file names.

---

## Parallelization guide

These tasks have **no file conflicts** and can be run by independent agents simultaneously:

| Agent | Tasks             | Files touched                                        |
| ----- | ----------------- | ---------------------------------------------------- |
| A     | 1a + 1b + 1c + 1d | `docs/guides/quick-start.md` only                    |
| B     | 2a + 2b           | `docs/guides/user-guide.md` only                     |
| C     | 3                 | `docs/guides/user-interface-overview.md` only        |
| D     | 4 + 5             | New files only (`getting-started.md`, `glossary.md`) |
| E     | 6                 | `README.md` only                                     |

All five agents can start at the same time with no merge conflicts.
