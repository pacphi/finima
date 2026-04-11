# finima-core

Domain models, types, error definitions, and repository traits for the Finima personal finance system.

## Purpose

This crate is the domain layer of Finima. It defines every domain entity (User, Account, Transaction, etc.), shared enum types, the application-wide error type, and the async repository trait interfaces that the database layer implements. All other crates in the workspace depend on `finima-core` for shared type definitions.

## Key Types / Modules

| Module                             | Description                                                                                                                                                               |
| ---------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `models/user.rs`                   | `User` -- identity, email, display name, preferences (JSON)                                                                                                               |
| `models/portfolio.rs`              | `Portfolio` -- top-level container grouping accounts                                                                                                                      |
| `models/account.rs`                | `Account` -- financial account with type, currency, opening balance                                                                                                       |
| `models/transaction.rs`            | `Transaction` -- individual financial transaction with category, merchant, confidence                                                                                     |
| `models/upload.rs`                 | `Upload` -- file import job with status tracking                                                                                                                          |
| `models/recurring_group.rs`        | `RecurringGroup` -- detected recurring payment pattern                                                                                                                    |
| `models/budget.rs`                 | `Budget` -- monthly spending target per category                                                                                                                          |
| `models/savings_goal.rs`           | `SavingsGoal` -- user-defined savings target                                                                                                                              |
| `models/account_flow.rs`           | `AccountFlow` -- money flow between accounts                                                                                                                              |
| `models/flow_group.rs`             | `FlowGroup` -- grouping of related flows                                                                                                                                  |
| `models/magic_link.rs`             | `MagicLink` -- passwordless auth token record                                                                                                                             |
| `models/session.rs`                | `Session` -- refresh token session                                                                                                                                        |
| `models/user_category_override.rs` | `UserCategoryOverride` -- user-defined categorization pattern                                                                                                             |
| `types.rs`                         | Shared enums: `AccountType` (12 variants), `Frequency` (8 variants), `UploadStatus`, `FileFormat` (8 variants) -- each with `Display`, `FromStr`, serde, and sqlx derives |
| `errors.rs`                        | `AppError` enum with variants for NotFound, Unauthorized, BadRequest, Conflict, DatabaseError, LlmError, ParseError; includes `From<sqlx::Error>` conversion              |
| `traits.rs`                        | Async repository traits: `UserRepo`, `PortfolioRepo`, `AccountRepo`                                                                                                       |

## Dependencies

This crate has no internal finima dependencies -- it is the root of the dependency graph. External dependencies include `rust_decimal` for monetary values, `sqlx` for database type derives, `chrono` for date/time, `uuid` for identifiers, and `serde`/`serde_json` for serialization.

The `axum` dependency is **optional** behind the `axum` feature flag. When enabled, `AppError` implements `IntoResponse` for direct use as an Axum handler return type.

## Developer Top-of-Mind

- **All monetary values use `rust_decimal::Decimal`** -- never `f64`. This prevents floating-point rounding errors in financial calculations.
- **Enum types require a full derive set**: every enum in `types.rs` must implement `Display`, `FromStr`, `Serialize`, `Deserialize`, and `sqlx::Type` with matching `rename_all = "snake_case"` conventions. Follow the existing pattern when adding new variants.
- **The `axum` feature flag** gates the `IntoResponse` impl on `AppError`. The API crate enables this; other crates do not need it.
- **`From<sqlx::Error>`** on `AppError` maps `RowNotFound` to `NotFound` and PostgreSQL unique-constraint violations (code `23505`) to `Conflict`. Keep this mapping in sync if new database error cases arise.
- **Repository traits** are defined here but implemented in `finima-db`. Adding a new repository method requires touching both crates.

## Testing

Run unit tests (enum roundtrips, error mappings, serde checks):

```sh
cargo test -p finima-core
```

Tests validate `Display`/`FromStr` roundtrips for all enum variants, serde JSON roundtrips, and verify `AppError` display strings and sqlx error conversions (including unique constraint mapping to `Conflict`). No database or network access is required.

## Architecture Notes

The dependency graph fans out from this crate: `finima-db` implements the traits, `finima-auth` converts errors, `finima-analysis` uses the domain types, and `finima-api` enables the `axum` feature for HTTP error responses. No crate in the workspace should bypass `finima-core` types by defining its own domain structs.
