# Quick Start Guide

Get Finima running on your machine in about ten minutes.

## Prerequisites

Before you begin, make sure you have the following installed:

- **Docker** (version 20.10 or later) and **Docker Compose** (v2).
  If you are on macOS or Windows, Docker Desktop includes both.
- **Git** for cloning the repository.
- **An email address** you can access. Finima uses magic-link
  sign-in, so you will receive a login link by email.

Optional but recommended:

- At least **8 GB of RAM** allocated to Docker. Ollama (the local AI
  model runner) benefits from extra memory.
- A GPU helps with AI categorization but is not required.

## 1. Clone the repository

```bash
git clone https://github.com/pacphi/finima.git
cd finima
```

## 2. Create your environment file

Copy the example file and open it in a text editor:

```bash
cp .env.example .env
```

Open `.env` and review these variables:

| Variable              | Purpose                                                                                                                                                   | Default      |
| --------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------ |
| `POSTGRES_PASSWORD`   | Password for the PostgreSQL database.                                                                                                                     | `finima_dev` |
| `RESEND_API_KEY`      | API key from [Resend](https://resend.com) for sending magic-link emails. Leave blank in development to have the link printed in the backend logs instead. | (empty)      |
| `MINIO_ROOT_USER`     | Username for the MinIO object store used for file uploads.                                                                                                | `finima`     |
| `MINIO_ROOT_PASSWORD` | Password for MinIO.                                                                                                                                       | `finima_dev` |

For a quick local setup you can leave every value at its default.
If you want to receive magic-link emails, sign up for a free
[Resend](https://resend.com) account and paste your API key into
`RESEND_API_KEY`.

## 3. Start the services

```bash
make docker-up
```

This starts three containers:

| Container         | Port        | Purpose                                     |
| ----------------- | ----------- | ------------------------------------------- |
| `finima-postgres` | 5432        | PostgreSQL 16 database                      |
| `finima-ollama`   | 11434       | Ollama LLM runtime for AI categorization    |
| `finima-minio`    | 9000 / 9001 | Object storage for uploaded bank statements |

Wait until `make docker-health` shows all containers as healthy.

## 4. Run the backend

In a separate terminal:

```bash
make dev
```

The backend API starts on `http://localhost:3000`.

## 5. Start the frontend

In another terminal:

```bash
make -C frontend dev
```

Or manually:

```bash
cd frontend
pnpm install
pnpm dev
```

The frontend starts on `http://localhost:5173`.

## 6. Sign in

1. Open your browser to `http://localhost:5173`.
2. Enter your email address and select **Send Magic Link**.
3. Check your inbox for an email from Finima and click the link.
   - If you did not set `RESEND_API_KEY`, look in the backend
     terminal output for a line containing the magic-link URL.
     Copy and paste it into your browser.
4. You are now signed in.

## 7. Complete onboarding

On your first login you will see a three-step setup wizard:

### Step 1 -- Profile

- Enter your **display name**.
- Choose your preferred **currency** (USD, EUR, GBP, CAD, AUD, or
  JPY).
- Choose your preferred **date format** (MM/DD/YYYY, DD/MM/YYYY, or
  YYYY-MM-DD).

### Step 2 -- Portfolio

A portfolio is a container that groups your accounts together. Most
people need only one.

- Give it a name (for example, "My Finances").
- Optionally add a short description.

### Step 3 -- First account

Add your first bank account:

- Pick an **account type**: Checking, Savings, Credit Card, Loan,
  Investment, Retirement, Cash, or Other.
- Enter a **name** (for example, "Chase Checking").
- Optionally enter the **institution** name.
- Set an **opening balance** if you know it.

You can also skip this step and create accounts later.

## 8. Import your first bank statement

1. Navigate to **Accounts** and select the account you just created.
2. On the account detail page, find the upload area.
3. Drag and drop a bank statement file, or click to browse.

Supported file formats:

| Format                     | Extensions             |
| -------------------------- | ---------------------- |
| Comma-separated values     | `.csv`, `.tsv`         |
| Open Financial Exchange    | `.ofx`, `.qfx`, `.qbo` |
| Quicken Interchange Format | `.qif`                 |
| Excel spreadsheet          | `.xls`, `.xlsx`        |

After uploading, Finima shows a **column mapping** screen where you
match the columns in your file to the fields Finima expects (date,
description, amount). Confirm the mapping and Finima imports your
transactions.

If Ollama is running and a model has been pulled, Finima
automatically categorizes each transaction using AI.

### Pull an AI model (optional)

To enable AI categorization, pull a model into Ollama:

```bash
make download-model
```

This downloads the default Gemma 4 model. The download is several
gigabytes and may take a few minutes.

> **Note:** If you skip this step, Finima runs in **stub mode** — all
> transactions will be categorized as "other" with a confidence of 0.5.
> The app is fully functional otherwise. You can pull a model later and
> re-import files to get AI categorization. See the
> [Troubleshooting Guide](troubleshooting.md#2a-degraded-mode--running-without-ollama)
> for details.

## 9. Explore the dashboard

Go to the **Dashboard** page. Once you have imported transactions you
will see:

- **Net Worth** -- a chart of your total net worth over time.
- **Financial Health** -- a score summarizing your overall financial
  health.
- **Cash Flow** -- monthly income vs. expenses.
- **Spending by Category** -- a donut chart of where your money goes.
- **Upcoming Bills** -- recurring payments due in the next 30 days.
- **Budget vs Actual** -- progress bars showing spending against your
  budget limits.

Dashboard widgets can be rearranged by dragging them.

## Next steps

- Read the [User Guide](user-guide.md) for a complete walkthrough of
  every feature.
- If something is not working, check the
  [Troubleshooting Guide](troubleshooting.md).
