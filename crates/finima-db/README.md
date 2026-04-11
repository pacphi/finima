# finima-db

PostgreSQL connection pooling, repository implementations, and database migrations for Finima.

## Purpose

This crate is the data-access layer. It implements the repository traits defined in `finima-core` using sqlx with PostgreSQL, manages the connection pool, and houses all SQL migration files. Every database query in the system flows through the `Pg*Repo` structs defined here.

## Key Types / Modules

| Module                       | Description                                                                                                                                                                                   |
| ---------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `pool.rs`                    | `create_pool()` -- builds a `PgPool` with configurable max connections                                                                                                                        |
| `repos/user_repo.rs`         | `PgUserRepo` -- user CRUD and email lookup                                                                                                                                                    |
| `repos/portfolio_repo.rs`    | `PgPortfolioRepo` -- portfolio CRUD with ownership verification                                                                                                                               |
| `repos/account_repo.rs`      | `PgAccountRepo` -- account CRUD, archival, balance computation                                                                                                                                |
| `repos/transaction_repo.rs`  | `PgTransactionRepo` -- transaction listing, filtering, pagination, sorting, bulk LLM updates; exports `Pagination`, `Sort`, `TransactionFilters`, `NewTransaction`, `LlmCategorizationUpdate` |
| `repos/upload_repo.rs`       | `PgUploadRepo` -- file upload tracking and status management                                                                                                                                  |
| `repos/session_repo.rs`      | `PgSessionRepo` -- session lifecycle and token revocation                                                                                                                                     |
| `repos/magic_link_repo.rs`   | `PgMagicLinkRepo` -- magic link storage and lookup                                                                                                                                            |
| `repos/recurring_repo.rs`    | `PgRecurringRepo` -- recurring group insert/update; exports `RecurringGroupInsert`, `RecurringGroupUpdate`                                                                                    |
| `repos/budget_repo.rs`       | `PgBudgetRepo` -- budget CRUD                                                                                                                                                                 |
| `repos/savings_goal_repo.rs` | `PgSavingsGoalRepo` -- savings goal CRUD                                                                                                                                                      |
| `repos/flow_repo.rs`         | `PgFlowRepo` -- account flow CRUD; exports `NewAccountFlow`                                                                                                                                   |
| `repos/flow_group_repo.rs`   | `PgFlowGroupRepo` -- flow group CRUD                                                                                                                                                          |
| `repos/override_repo.rs`     | `PgOverrideRepo` -- user category override management                                                                                                                                         |
| `migrations/`                | SQL migration files managed by sqlx                                                                                                                                                           |

## Dependencies

Depends on **finima-core** for domain models (`User`, `Account`, `Transaction`, etc.), error types (`AppError`), and enum types (`AccountType`, `Frequency`, etc.) used in query result mapping.

## Developer Top-of-Mind

- **All queries use parameterized binds** (`$1`, `$2`, ...) -- never string interpolation. This is critical for SQL injection prevention.
- **Sort fields are whitelisted**: the `Sort` struct in `transaction_repo` only allows a predefined set of column names to prevent injection through ORDER BY clauses.
- **`per_page` is capped at 100** in pagination to prevent clients from fetching unbounded result sets.
- **`session_repo` handles token revocation**: when a session is deleted or a refresh token is rotated, old tokens are invalidated in the database.
- Each `Pg*Repo` wraps a `PgPool` clone. Repos are constructed once in `AppState` and shared across all handlers.
- When adding new queries, follow the existing pattern of `sqlx::query_as!` or `sqlx::query!` macros with explicit type annotations.

## Testing

```sh
cargo test -p finima-db
```

Integration tests require a running PostgreSQL instance. Set `DATABASE_URL` in your environment or `.env` file. Run migrations first with `sqlx migrate run`.

## Migrations

Migration files live in `src/migrations/` and are executed by sqlx at startup. When adding a new migration:

1. Create the file with `sqlx migrate add <name>`
2. Write idempotent SQL (use `IF NOT EXISTS`, `CREATE OR REPLACE` where possible)
3. Test both the up-migration and a clean `migrate run` from scratch

## Architecture Notes

All repos follow the same structural pattern: a newtype wrapping `PgPool`, constructed via `::new(pool)`, with async methods returning `Result<T, AppError>`. Query results are mapped directly into `finima-core` domain models. There is no ORM -- all SQL is hand-written for clarity and performance control.
