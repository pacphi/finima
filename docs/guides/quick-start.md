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

Open `.env` and review the variables. The file is self-documented.
For a quick local setup the defaults work out of the box.

**Key variables:**

| Variable                | Purpose                                                      | Default                                              |
| ----------------------- | ------------------------------------------------------------ | ---------------------------------------------------- |
| `POSTGRES_PASSWORD`     | PostgreSQL password (used by Docker)                         | `finima_dev`                                         |
| `APP__DATABASE__URL`    | Full DB connection string (must match `POSTGRES_PASSWORD`)   | `postgres://finima:finima_dev@localhost:5432/finima` |
| `APP__AUTH__JWT_SECRET` | JWT signing secret (generate with `openssl rand -base64 32`) | placeholder                                          |
| `APP__RESEND__API_KEY`  | [Resend](https://resend.com) API key for magic-link emails   | (empty)                                              |
| `APP__AUTH__FROM_EMAIL` | Sender address for magic-link emails                         | `Finima <auth@finima.app>`                           |
| `APP__AUTH__PUBLIC_URL` | Base URL for magic-link emails (must be the frontend origin) | `http://localhost:5173`                              |
| `MINIO_ROOT_USER`       | MinIO username (used by Docker)                              | `finima`                                             |
| `MINIO_ROOT_PASSWORD`   | MinIO password (used by Docker)                              | `finima_dev`                                         |

If you change `POSTGRES_PASSWORD`, update the password in
`APP__DATABASE__URL` to match.

> **Do not quote values.** Both Make and Docker Compose treat
> quotes as literal characters. Write `POSTGRES_PASSWORD=secret`,
> not `POSTGRES_PASSWORD="secret"`.

### Email setup

Finima uses [Resend](https://resend.com) to deliver magic-link
sign-in emails. Three variables control email delivery:

| Variable                | What it does                                                                                                           |
| ----------------------- | ---------------------------------------------------------------------------------------------------------------------- |
| `APP__RESEND__API_KEY`  | Resend API key. When set, real emails are sent. When empty, magic links are logged to the terminal instead.            |
| `APP__AUTH__FROM_EMAIL` | Sender address shown in the email. Must be on a verified Resend domain (or use `onboarding@resend.dev` for testing).   |
| `APP__AUTH__PUBLIC_URL` | Base URL embedded in magic-link emails. Must point to the frontend origin so the link opens in your browser correctly. |

**Development (no verified domain):**

```dotenv
APP__RESEND__API_KEY=re_your_key_here
APP__AUTH__FROM_EMAIL=Finima <onboarding@resend.dev>
APP__AUTH__PUBLIC_URL=http://localhost:5173
```

Using `onboarding@resend.dev` sends real emails without domain
verification, but delivery is limited to the email address on your
Resend account.

**Production (verified domain):**

```dotenv
APP__RESEND__API_KEY=re_live_key_here
APP__AUTH__FROM_EMAIL=Finima <auth@finima.app>
APP__AUTH__PUBLIC_URL=https://finima.app
```

Verify the `finima.app` domain in your
[Resend dashboard](https://resend.com/domains) first (add the DNS
records Resend provides, then wait for verification).

**No Resend key (log-only mode):**

Leave `APP__RESEND__API_KEY` empty. Magic links are printed in the
backend terminal with a `[DEV]` prefix. Copy the URL and paste it
into your browser.

## 3. Start everything

### Option A — Using Make (recommended)

```bash
make start
```

This starts the infrastructure containers, waits for them to be
healthy, then launches the backend and frontend together. Press
`Ctrl-C` to stop.

You can also start infrastructure and the app separately:

```bash
make docker-infra       # Start infrastructure containers
make dev                # Start backend + frontend
```

### Option B — Without Make

```bash
# Start infrastructure
docker compose up -d

# In one terminal — start the backend
APP_ENV=development cargo run --bin finima-api

# In another terminal — start the frontend
cd frontend
pnpm install
pnpm dev
```

The backend loads `.env` automatically via `dotenvy`, so `APP__*`
environment variables work regardless of how you start the app.

### Infrastructure containers

| Container         | Port        | Purpose                                     |
| ----------------- | ----------- | ------------------------------------------- |
| `finima-postgres` | 5432        | PostgreSQL 16 database                      |
| `finima-ollama`   | 11434       | Ollama LLM runtime (only when `LLM=ollama`) |
| `finima-minio`    | 9000 / 9001 | Object storage for uploaded bank statements |

### Application services

| Service  | URL                   |
| -------- | --------------------- |
| Backend  | http://localhost:3000 |
| Frontend | http://localhost:5173 |

With `make dev`, both run in the same terminal. With `make
dev-backend`, only the backend starts (useful while working on
backend code).

## 6. Sign in

1. Open your browser to `http://localhost:5173`.
2. Enter your email address and select **Send Magic Link**.
3. Check your inbox for an email from Finima and click the link.
   - If you did not set `APP__RESEND__API_KEY`, Finima logs the
     magic-link URL to the backend terminal instead of sending
     email. Look for a `[DEV]` log line containing
     `/auth/verify?token=` and paste the full URL into your browser.
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

### AI categorization

By default, `make start` and `make dev` compile the backend with the
**Candle** LLM backend. The Makefile auto-detects your hardware
(Metal on macOS, CUDA on NVIDIA, CPU otherwise). On first startup
the model downloads from HuggingFace (~4-5 GB).

If you prefer to use **Ollama** instead (HTTP-based, runs in Docker):

```bash
make start LLM=ollama     # starts Ollama container + pulls model
make download-model LLM=ollama   # pull the default Gemma 4 model
```

To run **without any LLM** (stub mode):

```bash
make start LLM=stub
```

See the `.env.example` file and the
[Maintainer Guide](maintainer-guide.md#llm-backend-configuration)
for full configuration details (model selection, quantization,
device, etc.).

> **Note:** In stub mode all transactions are categorized as "other"
> with a confidence of 0.5. The app is fully functional otherwise.
> You can switch to a real LLM later and re-import files to get AI
> categorization. See the
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
