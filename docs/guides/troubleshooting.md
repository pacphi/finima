# Troubleshooting Guide

Solutions for common issues when running Finima. If your problem is
not listed here, check the backend logs with `make docker-infra-logs` for
error messages.

---

## 1. Cannot receive magic-link email

**Symptoms:** You enter your email on the sign-in page, see the
"Magic link sent" confirmation, but never receive the email.

**Possible causes and fixes:**

- **`APP__RESEND__API_KEY` is not set.** Finima sends real emails
  only when this key is configured. If you left it blank in your
  `.env` file, the magic-link URL is printed in the backend terminal
  output instead. Look for a `[DEV]` log line containing
  `/auth/verify?token=`. Copy the full URL and paste it into your
  browser.

- **Email is in your spam folder.** Check your spam or junk folder
  for an email from "Finima" or the address configured in
  `config/default.yaml` under `auth.from_email`.

- **Rate limit reached.** Finima limits magic-link requests to 5 per
  minute per IP address. Wait a minute and try again.

- **Invalid API key.** Verify that the `APP__RESEND__API_KEY` in
  your `.env` file is correct. Log in to your Resend dashboard to
  confirm the key is active. The backend logs will show a Resend API
  error if the key is rejected.

- **Domain not verified (403 Forbidden).** If the backend logs show
  `"The ... domain is not verified"`, the sender domain in
  `APP__AUTH__FROM_EMAIL` has not been verified in Resend. Either
  verify the domain at <https://resend.com/domains>, or use
  `APP__AUTH__FROM_EMAIL=Finima <onboarding@resend.dev>` for local
  testing (delivers only to your Resend account email).

- **Magic link opens a blank page or 404.** The link URL is built
  from `APP__AUTH__PUBLIC_URL`. If this is set to the wrong host
  (e.g. `0.0.0.0` or a backend address), the link will not reach
  the frontend. Set it to the frontend origin, e.g.
  `APP__AUTH__PUBLIC_URL=http://localhost:5173` for local dev or
  `APP__AUTH__PUBLIC_URL=https://finima.app` for production.

---

## 2. Transactions not categorized by AI

**Symptoms:** You import transactions but the category column is
empty or shows generic values.

**Possible causes and fixes:**

- **Ollama is not running.** Check with `make docker-infra-ps`. If the
  `finima-ollama` container is not listed or is unhealthy, restart
  services with `make docker-infra-restart`.

- **No model has been pulled.** Ollama needs a model downloaded
  before it can categorize anything. Run:

  ```bash
  make download-model LLM=ollama
  ```

  This downloads the default Gemma 4 model (several gigabytes).

- **Wrong Ollama URL.** The backend expects Ollama at
  `http://localhost:11434`. If you changed the port, update
  `config/default.yaml` under `llm.ollama.url`.

- **Check the LLM tab in Settings.** Go to **Settings > LLM** and
  verify the connection status shows "Connected". If it shows
  "Disconnected", the backend cannot reach Ollama.

- **Re-categorize existing transactions.** AI categorization runs
  at import time. Transactions imported before Ollama was available
  will not be retroactively categorized. Re-import the same file to
  pick up uncategorized transactions.

---

## 2a. Degraded Mode — Running Without Ollama

If Ollama is not running, no model has been pulled, or the `ollama`
compile-time feature flag is disabled, Finima silently falls back to a
**stub LLM client**. In this mode:

- All transaction categorization returns `category="other"` with
  `confidence=0.5`.
- All enrichment returns default/empty values.
- All insight generation returns a placeholder string.
- Feed article summarization returns stubs.

This is graceful degradation by design — the application remains fully
functional for importing, viewing, and manually categorizing
transactions.

**How to detect stub mode:**

- Check the backend logs at startup for the line:
  `Using STUB LLM client`
- In the UI, go to **Settings > LLM**. If the connection status shows
  "Disconnected", the stub client is active.

**Re-categorizing after enabling Ollama:**

Transactions imported during stub mode will have generic categories.
Once Ollama is running with a model pulled, re-import the same file
to pick up AI categorization for those transactions.

---

## 3. Upload fails

**Symptoms:** You try to upload a bank statement and see an error
message.

**Possible causes and fixes:**

- **File too large.** The maximum upload size is 50 MB. If your
  statement is larger, split it into smaller date ranges.

- **Unsupported format.** Finima supports CSV, TSV, OFX, QFX, QBO,
  QIF, XLS, and XLSX. Other formats (PDF, images, etc.) are not
  supported. Export a CSV from your bank's website instead.

- **MinIO is not running.** File uploads are stored in MinIO before
  being processed. Check with `make docker-infra-ps` that the
  `finima-minio` container is running and healthy. If not:

  ```bash
  make minio
  ```

- **MinIO credentials mismatch.** If you changed `MINIO_ROOT_USER`
  or `MINIO_ROOT_PASSWORD` in `.env`, make sure the matching values
  are in `config/default.yaml` under `s3.access_key_id` and
  `s3.secret_access_key`.

---

## 4. Dashboard shows zeros or empty widgets

**Symptoms:** The dashboard loads but widgets show "No data yet"
messages or zero values.

**Possible causes and fixes:**

- **No transactions imported yet.** Dashboard data comes from your
  transaction history. Import at least one bank statement to see
  data populate.

- **Wrong date range.** Some widgets show data for the current month.
  If all your transactions are from a different month, the current
  month will appear empty. Use the Transactions page to verify your
  data, then check the Budget or Flows pages which have month
  navigation.

- **No budgets set.** The "Budget vs Actual" widget requires at
  least one budget entry. Go to the **Budget** page and create one
  or use **Auto-Suggest**.

- **No primary income account.** The Upcoming Bills widget and some
  Flow calculations require at least one account to be marked as a
  primary income source.

---

## 5. Session expires frequently

**Symptoms:** You get signed out and redirected to the sign-in page
frequently.

**This is expected behavior.** Access tokens expire after 15 minutes
for security. The frontend automatically refreshes your session using
a refresh token without requiring you to sign in again.

If you are being signed out completely (not just briefly), check:

- **Browser privacy settings.** Some browsers or extensions block
  cookies or local storage, which prevents the refresh token from
  being stored.

- **Clock skew.** If your system clock is significantly wrong, token
  validation may fail. Make sure your computer's time is synced.

---

## 6. Docker services will not start

**Symptoms:** Running `make docker-infra` fails, or containers
immediately exit.

**Possible causes and fixes:**

- **Port conflicts.** Finima uses ports 3000 (backend), 5173
  (frontend), 5432 (PostgreSQL), 9000/9001 (MinIO), and 11434
  (Ollama). If another application is using one of these ports, the
  container will fail to start. Stop the conflicting application or
  change the port mapping in `docker-compose.yml`.

  To find what is using a port:

  ```bash
  lsof -i :5432
  ```

- **Not enough memory.** Ollama in particular can require significant
  memory. Open Docker Desktop and increase the memory limit to at
  least 8 GB under Settings > Resources.

- **Check container logs.** Run `make docker-infra-logs` to see error
  output from all containers. For a specific container:

  ```bash
  docker compose logs postgres
  docker compose logs ollama
  docker compose logs minio
  ```

- **GPU driver issues (Ollama).** The `docker-compose.yml` file
  requests GPU access for Ollama. If you do not have an NVIDIA GPU,
  Ollama still works on CPU but you may need to remove the `deploy`
  section from the Ollama service in `docker-compose.yml`.

---

## 7. Database connection errors

**Symptoms:** The backend logs show errors about connecting to
PostgreSQL, or pages fail to load with server errors.

**Possible causes and fixes:**

- **PostgreSQL container is not running.** Check with
  `make docker-infra-ps`. If it is not healthy, restart with
  `make docker-infra-restart`.

- **Password mismatch.** The database password is set by the
  `POSTGRES_PASSWORD` environment variable in `.env` (default:
  `finima_dev`). The backend reads `database.password` from
  `config/default.yaml` (overridable via `APP__DATABASE__PASSWORD`).
  Make sure the password in both places matches.

- **Migrations have not been run.** If you see errors about missing
  tables, run:

  ```bash
  make migrate
  ```

- **Database volume is corrupted.** As a last resort, run
  `make docker-infra-down && docker volume rm finima_pgdata` then
  `make docker-infra && make migrate`. This deletes all data.

---

## 8. Slow performance

**Symptoms:** Pages take a long time to load, or AI categorization
is very slow.

**Possible causes and fixes:**

- **Ollama model loading.** The first request after starting Ollama
  takes longer because the model needs to load into memory. After
  the first request, subsequent categorizations are faster.

- **Increase Docker resources.** Open Docker Desktop and allocate
  more CPU and memory under Settings > Resources.

- **Too many database connections.** The default connection pool is
  10 connections. If you see connection pool timeout errors in the
  backend logs, increase `database.max_connections` in
  `config/default.yaml`.

- **Large import files.** Very large statement files (tens of
  thousands of transactions) take longer to process. Consider
  importing smaller date ranges.

---

## 9. Currency or dates show in the wrong format

**Symptoms:** Amounts show the wrong currency symbol or dates are in
an unexpected order.

**Possible causes and fixes:**

- **Check Settings.** Go to **Settings > General** and verify the
  currency and date format are set correctly. Select **Save
  Preferences** after making changes.

- **Clear browser cache.** Preferences are cached locally. If your
  settings seem to revert, clear your browser's local storage for
  the Finima site and reload the page.

- **Per-account currency.** Each account has its own currency
  setting. If a specific account shows the wrong currency, check
  the account's configuration on the Accounts page.

---

## 10. How to reset everything and start fresh

If you want to wipe all data and start over:

```bash
make clean-all
```

This command:

1. Cleans all build artifacts.
2. Stops all Docker containers.
3. Deletes all Docker volumes, including the database and uploaded
   files.

After running this, start again from step 3 of the
[Quick Start](quick-start.md) guide.

To reset only the database without removing other data:

```bash
make docker-infra-down
docker volume rm finima_pgdata
make docker-infra
make migrate
```

To load sample data for testing:

```bash
make db-seed
```

This loads test seed data into the database. It only works in
development and test environments (not production).

---

## Getting more help

- **Backend logs:** `make docker-infra-logs`
- **Container status:** `make docker-infra-ps` or `make docker-infra-health`
- **Check configuration:** Review `config/default.yaml` for all
  server-side settings.
- **Quick Start guide:** [quick-start.md](quick-start.md)
- **User Guide:** [user-guide.md](user-guide.md)
