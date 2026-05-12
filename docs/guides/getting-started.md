# Your First Week with Finima

Welcome! This guide is written for anyone who has never installed software from a terminal before. If you can follow a recipe, you can set up Finima. Set aside about 15–20 minutes and you will have your own personal finance app running on your computer by the end.

---

## What Finima does — and what you need

Finima is a self-hosted personal finance app. "Self-hosted" means it runs on your own computer, not on someone else's server. Your financial data never leaves your machine.

With Finima you can:

- Track bank accounts, credit cards, loans, and investments in one place.
- Import transaction history from files you download from your bank.
- See your net worth, spending by category, cash flow, and more on a dashboard.
- Set budgets and track whether you are sticking to them.

**What you need before you start:**

| Item                                        | Notes                                                                     |
| ------------------------------------------- | ------------------------------------------------------------------------- |
| A computer running macOS, Windows, or Linux | Any reasonably modern machine works.                                      |
| An email address you can access right now   | Finima uses a magic-link sign-in — no password needed.                    |
| 15–20 minutes (first time)                  | Most of this is waiting for downloads.                                    |
| At least one bank statement file            | You can skip this for now and add it later, but it is good to have ready. |

---

## Step 1: Install Docker

Docker is a tool that runs Finima (and everything it depends on) inside tidy, isolated containers. Think of it as a self-contained box — no database to install separately, no programming languages to set up. Docker handles all of that.

| Operating system | How to install                                                                                                                                                                                                                                                                                                                                                                                                   |
| ---------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Mac**          | Go to [https://www.docker.com/products/docker-desktop/](https://www.docker.com/products/docker-desktop/) and download the installer for your chip (Apple silicon or Intel — check Apple menu > About This Mac if unsure). Open the downloaded `.dmg` file, drag Docker to your Applications folder, and launch it. Wait for the small whale icon in the menu bar to stop animating — that means Docker is ready. |
| **Windows**      | Go to [https://www.docker.com/products/docker-desktop/](https://www.docker.com/products/docker-desktop/) and download the Windows installer. Run it and accept the default WSL 2 backend option (this is the modern Windows subsystem that Docker needs). Restart your computer if prompted, then open Docker Desktop from the Start menu.                                                                       |
| **Linux**        | Follow the instructions for your specific distribution at [https://www.docker.com/products/docker-desktop/](https://www.docker.com/products/docker-desktop/). After installation, start the Docker service and add your user to the `docker` group so you can run Docker without `sudo`.                                                                                                                         |

> **You do not need a Docker account.** Docker Desktop works without signing in. If it asks you to create an account or sign in, you can dismiss or skip that prompt.

---

## Step 2: Install Git

Git is a tool for downloading and managing code. You only need one command from it in this guide.

| Operating system          | How to install                                                                                                                                                                                                                                                                                                   |
| ------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Mac**                   | Open the **Terminal** app (search for "Terminal" in Spotlight — press `⌘ Space` and type "Terminal"). Type `git --version` and press Enter. If Git is not already installed, macOS will offer to install the Xcode Command Line Tools. Click **Install** and follow the prompts. When it finishes, Git is ready. |
| **Windows**               | Go to [https://git-scm.com/download/win](https://git-scm.com/download/win) and download the installer. Run it and accept all the default options. When it finishes you will have a program called **Git Bash** — use that whenever this guide says "open a terminal."                                            |
| **Linux (Debian/Ubuntu)** | Open a terminal and run `sudo apt install git`.                                                                                                                                                                                                                                                                  |
| **Linux (Fedora/RHEL)**   | Open a terminal and run `sudo dnf install git`.                                                                                                                                                                                                                                                                  |

---

## Step 3: Download Finima

Open a terminal (on Mac and Linux this is the Terminal app; on Windows use Git Bash) and run these two commands. You can copy and paste them:

```bash
git clone https://github.com/pacphi/finima.git && cd finima
```

This downloads Finima into a folder called `finima` inside your current directory and then moves into that folder. The download usually takes less than a minute.

---

## Step 4: Configure and start Finima

### Create your configuration file

Run this command to create your personal configuration file from the template:

```bash
cp .env.example .env
```

This copies the example settings file to `.env`. The defaults are fine for a local setup — you do not need to edit anything to get started.

### Start Finima

Run:

```bash
make start
```

**The first run takes 5–15 minutes.** Docker needs to download the container images (the software packages Finima depends on) from the internet. This only happens once. After the first run, starting Finima takes under a minute.

You will see a lot of text scrolling in your terminal. That is normal — Docker and the app are printing status messages. When you see both of these lines (they may not appear one right after the other), Finima is ready:

```
finima-api listening on 0.0.0.0:3000
Local:   http://localhost:5173/
```

> **Note on sign-in emails:** By default, Finima does not send real emails — it runs in "log-only mode." When you request a magic link, the sign-in URL is printed directly in your terminal instead of arriving in your inbox. Step 5 below explains exactly what to look for.

---

## Step 5: Sign in for the first time

1. Open your web browser and go to **[http://localhost:5173](http://localhost:5173)**.
2. You will see the Finima sign-in page. Type in your email address and click **Send magic link**.
3. Switch back to your terminal window. Look for a line that starts with `[DEV]` and contains a URL. It looks something like this:

   ```
   [DEV] Magic link: http://localhost:5173/auth/verify?token=abc123...
   ```

4. Copy the full URL (starting from `http://`) and paste it into your browser's address bar. Press Enter.
5. You are now signed in and will be taken through the setup wizard.

> **Having trouble?** See the [Troubleshooting guide](troubleshooting.md#1-cannot-receive-magic-link-email) for solutions to common sign-in issues.

---

## Step 6: Set up your profile

After signing in for the first time, Finima walks you through a three-step setup wizard.

**Step 1 — Profile:** Enter your display name, choose your home currency (for example USD, EUR, or GBP), and select the date format you prefer (MM/DD/YYYY or DD/MM/YYYY).

**Step 2 — Portfolio:** A portfolio is a named collection of accounts. Most people only need one. Give it a simple name like "Personal" or "My Finances." (You can add more portfolios later if you ever want to separate personal and business finances.)

**Step 3 — First account:** Choose the account type (Checking, Savings, Credit Card, and so on) and give it a recognizable name like "Chase Checking" or "Visa Rewards." You can add more accounts later.

Once you complete the wizard you land on your dashboard.

---

## Step 7: Import your first bank statement

### How to get a bank statement from your bank

Finima does not connect to your bank directly — it never needs your bank username or password. Instead, you download a file from your bank's website and upload it to Finima.

**General instructions:** Log in to your bank's website and look for wording like:

- "Download Activity"
- "Export Transactions"
- "Statement Download"

This is usually found in the account history or statements section. When given a choice of format, pick **CSV** — it is the most widely supported and easiest to work with.

**Where to find the export option at major US banks:**

| Bank                | Where to look                                                                                                                                                |
| ------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **Chase**           | Open an account, scroll to transaction history, click **Download account activity** (top-right of the transaction list), choose a date range and CSV format. |
| **Bank of America** | Go to account activity, click **Download** (near the search/filter area), choose a date range and the "Microsoft Excel" or "CSV" format option.              |
| **Wells Fargo**     | Open an account, click **Download Activity** above the transaction list, choose your date range and select "Comma-Delimited File (CSV)."                     |
| **Capital One**     | Go to account activity, click the **Download** icon (arrow pointing down), choose your date range and "CSV (Excel)" format.                                  |
| **Citi**            | Go to account activity, select **Download** or **Export**, choose a date range, and pick "CSV."                                                              |

Save the downloaded file somewhere you can find it easily (your Desktop works fine).

### Upload the file to Finima

1. In Finima, go to the **Accounts** page and click the account you want to import transactions into.
2. On the account detail page, find the upload area. You can either drag and drop your file onto it, or click to browse your computer and select the file.
3. Finima reads the file and shows a **column mapping** screen. This lets you tell Finima which column in your file is the date, which is the description, and which is the amount. (For OFX, QFX, QBO, and QIF files this step is skipped because those formats are self-describing.)
4. Review the mapping, then click **Confirm**. Finima imports the transactions.

Finima automatically suggests a category for each transaction (for example, "Groceries" or "Streaming"). You can change any of them by clicking the category on the Transactions page.

---

## Step 8: Explore the dashboard

Head back to the dashboard (click the Finima logo or the Home link). You will find six widgets:

| Widget                     | What it shows                                                                                                               |
| -------------------------- | --------------------------------------------------------------------------------------------------------------------------- |
| **Net Worth**              | Your total assets minus your total liabilities, shown as a number and a chart over time.                                    |
| **Financial Health Score** | A gauge that rates your overall financial health, taking into account your savings rate, debt levels, and budget adherence. |
| **Cash Flow**              | A bar chart comparing your monthly income and expenses side by side over the past twelve months.                            |
| **Spending by Category**   | A donut chart breaking your spending into categories for the current month. Click a slice to see those transactions.        |
| **Upcoming Bills**         | Recurring payments Finima has detected from your transaction history, shown with their expected date and amount.            |
| **Budget vs Actual**       | Progress bars showing how much you have spent against each budget you have set up.                                          |

> **Don't worry if the charts look sparse.** One month of data is not enough to show meaningful trends. The more history you import, the more useful the dashboard becomes.

---

## What's next

Now that you are set up, here are some good next steps:

- **Add more accounts** — go to Accounts > + Add Account to add credit cards, savings accounts, or loans.
- **Import more history** — most banks let you export up to 18–24 months of transactions. The more data you have, the better your charts and score will be.
- **Set up budgets** — go to the Budgets page. Finima will suggest budget amounts based on your recent spending.
- **Read the full user guide** — the [User Guide](user-guide.md) covers every feature in detail, including recurring payment detection, multi-portfolio management, and the Sankey diagram view.
- **Troubleshoot issues** — if something is not working as expected, check the [Troubleshooting guide](troubleshooting.md).

---

## Stopping and restarting Finima

**To stop Finima:** Switch to the terminal where Finima is running and press `Ctrl-C`. Docker will shut down the containers gracefully.

**To start Finima again later:** Open a terminal, navigate to the finima folder, and run:

```bash
cd finima && make start
```

(Replace `finima` with the full path to the folder if needed, for example `cd ~/Documents/finima && make start`.)

**Your data is safe.** Finima stores all data in a local database managed by Docker. Stopping and restarting Finima does not delete anything. Your accounts, transactions, budgets, and settings are all still there when you come back.

---

## New to some of the words used here?

See the [Glossary](glossary.md) for plain-English definitions of financial terms (net worth, cash flow, budget) and app terms (magic link, portfolio, Sankey diagram).
