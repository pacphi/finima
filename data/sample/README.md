# Sample Dataset

A committed, reproducible demo fixture modeling a joint-household portfolio
over 18 months working back from **2026-04-20**. Intended for local demos,
dashboard screenshots, and manual QA — **never loaded in production**
(excluded from Docker images via `.dockerignore`).

Separate lifecycle from `tests/seed.sql` (which is integration-test only).

## Portfolio

| Account          | Type                   | Institution      |
| ---------------- | ---------------------- | ---------------- |
| Joint Checking   | `checking`             | Chase Bank       |
| Joint Savings    | `savings`              | Chase Bank       |
| Amex Reserve     | `credit_card`          | American Express |
| Atmos Visa       | `credit_card`          | Atmos            |
| Mazda CX-90 Loan | `loan_auto`            | Mazda Financial  |
| Schwab Brokerage | `investment_brokerage` | Charles Schwab   |

Fixed identifiers:

- User: `a2000000-0000-4000-8000-000000000001` (`sample@finima.local`)
- Portfolio: `b2000000-0000-4000-8000-000000000001` (`Sample Household`)
- Savings goal: Australia Round-Trip, target $14,000, due 2026-11-15

## Prerequisites

`make sample-load` / `sample-purge` shell out to the `psql` CLI; if it's not
on your `PATH` these targets will fail with `command not found`. The server
runs PostgreSQL 16, so pair the client to match — `psql` is
forward-compatible but mismatched majors occasionally mis-render
server-side notices.

- **Minimum:** `psql` 14 (works against PG 16 for all fixture operations).
- **Recommended:** `psql` 16 — same major as the server.

Install one of the following (choose one — do not stack them):

```bash
# Option A — Homebrew (macOS): client + server, auto-PATH
brew install postgresql@16
brew link --force postgresql@16         # expose psql on $PATH

# Option A' — Homebrew, client only (no local server)
brew install libpq
brew link --force libpq                 # exposes psql without the server

# Option B — mise (preferred for pinned, per-project toolchains)
mise use -g postgres@16                 # global
# or, from the repo root:
mise use postgres@16                    # pins to .mise.toml in cwd
```

Verify:

```bash
psql --version                          # expect: psql (PostgreSQL) 16.x
```

Docker users can skip the host install and exec into the running container
instead: `docker compose exec postgres psql -U finima finima < data/sample/sample.sql`.

## Usage

```bash
make sample-load                   # load fixture into current DATABASE_URL
make sample-attach EMAIL=you@real  # re-own the sample portfolio to an existing user
make sample-purge                  # remove the sample portfolio + user
make sample-regen                  # regenerate data/sample/sample.sql deterministically
```

`sample-load` is idempotent (`ON CONFLICT DO NOTHING`) — re-running it is safe.

## Logging in

The sample fixture creates its own user (`sample@finima.local`) that owns
the `Sample Household` portfolio. Magic-link auth emails the login URL to
the user's inbox, and `sample@finima.local` is fake — you can't receive it
via Resend. Two ways around that:

### Option A — stdout magic-link (keep the sample user, disable Resend)

Restart the backend with Resend disabled so the auth layer falls back to
`LoggingEmailSender` (`crates/finima-auth/src/resend.rs`), which prints the
magic link to stdout:

```bash
APP__RESEND__API_KEY="" make dev-backend
```

Request a link for the sample user:

```bash
curl -X POST http://localhost:3000/api/auth/magic-link \
  -H 'Content-Type: application/json' \
  -d '{"email":"sample@finima.local"}'
```

Backend log shows:

```text
[DEV] Magic link for sample@finima.local: http://localhost:5173/auth/verify?token=...&email=sample%40finima.local
```

Open that URL in a browser — VerifyPage consumes the token, issues JWTs,
stores them in `sessionStorage['finima-auth']`, and drops you on the
dashboard with the sample portfolio loaded.

**Pros:** zero code changes, sample data stays fully isolated (different
user, different portfolio).
**Cons:** you're logged in as the sample user, not yourself. Logging back
in as your real user means another magic-link round.

### Option B — `sample-attach` (keep Resend on, re-own to your real user)

Log in once through the normal flow with your real email so the `users`
row exists, then:

```bash
make sample-attach EMAIL=you@real.com
```

This runs a single transaction that:

1. Verifies `you@real.com` already exists in `users` (email lookup — no
   UUIDs).
2. Verifies the sample fixture is loaded (`sample@finima.local` exists).
3. `UPDATE` the portfolio named `Sample Household` currently owned by
   `sample@finima.local` → new `user_id` from the real-user email lookup.
4. `DELETE FROM users WHERE email = 'sample@finima.local'` — the
   now-orphaned sample user.

After that, logging in with `you@real.com` lists both your real
portfolio(s) and `Sample Household` side-by-side.

**Pros:** keep Resend enabled, real email flow unchanged.
**Cons:** the sample portfolio is now entangled with your real user —
`sample-purge` still cleans it up cleanly, so your real user survives
purge.

### Identification strategy

`sample-attach` and `sample-purge` never reference UUIDs directly. They
locate sample rows by their semantic markers:

- User: `email = 'sample@finima.local'`
- Portfolio: `name = 'Sample Household'`

`sample-purge` handles both lifecycle states — whether the portfolio still
belongs to the sample user or has been reparented via `sample-attach`:

```sql
DELETE FROM portfolios WHERE name = 'Sample Household';
DELETE FROM users      WHERE email = 'sample@finima.local';
```

(The UUIDs still exist in the generated `sample.sql` — they're required
for cross-row FK references at insert time — but they're an internal
detail. Tests that need to reference rows by id can still rely on them as
stable constants; everyday maintainer operations go through the semantic
lookups above.)

## Files

- `sample.sql` — generated SQL fixture, committed.
- Generator source: `crates/finima-api/src/bin/generate_sample.rs`.

## Design notes

- **Amount strategy** is mixed: recurring/fixed costs are pinned to round
  numbers (payroll $6,400 biweekly, rent $2,900, Mazda $625/mo, etc.);
  variable spend (groceries, dining, gas, shopping) is sampled from clipped
  distributions (p5–p95) so there are no extreme swings.
- Transaction `direction` is populated per ADR-018 (`inflow` / `outflow`).
- Recurring groups are pre-confirmed (`is_confirmed = true`), so the
  recurring UI shows them immediately without needing a detection pass.
- Account flows are seeded for checking↔savings, checking→Schwab, and
  credit-card payoffs so the Sankey view renders without a flow-detection
  run.
- Budgets exist for all 18 months across six categories so historical
  budget-vs-actual charts render with real history.
- The generator is seeded with a fixed splitmix64 value; reruns produce a
  byte-identical `sample.sql` (use `git diff` as the regression check).
