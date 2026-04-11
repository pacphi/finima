# finima-auth

Passwordless authentication system implementing magic links, JWT management, email delivery, and Axum middleware.

## Purpose

This crate implements the authentication flow described in ADR-002. It handles the full passwordless lifecycle: generating magic link tokens, sending emails via the Resend API, encoding/decoding JWTs for access and refresh tokens, and providing an Axum extractor that protects routes. It converts its own `AuthError` into `finima-core::AppError` so handlers can use `?` seamlessly.

## Key Types / Modules

| Module          | Description                                                                                                                                                                                     |
| --------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `jwt.rs`        | `encode_access_token`, `encode_refresh_token`, `decode_access_token`, `decode_refresh_token`; defines `Claims` and `TokenType` (Access vs Refresh) with type-claim discrimination               |
| `magic_link.rs` | `generate_token()` for cryptographic random tokens, `hash_token()` for SHA-256 hashing, `build_magic_link_url()` for constructing the verification URL                                          |
| `middleware.rs` | `AuthUser` extractor -- pulls Bearer token from Authorization header, decodes via `decode_access_token`, yields `user_id` and `email`; `AuthRejection` for error responses; `JwtSecret` newtype |
| `resend.rs`     | `EmailSender` trait, `ResendClient` (production Resend API integration), `LoggingEmailSender` (dev/test double that logs instead of sending)                                                    |
| `lib.rs`        | `AuthError` enum with `TokenEncoding`, `TokenDecoding`, `EmailDelivery`, `InvalidMagicLink` variants; `From<AuthError> for AppError` conversion                                                 |

## Dependencies

Depends on **finima-core** for the `AppError` type (auth errors convert into it). Uses `jsonwebtoken` for JWT operations, `sha2` for token hashing, `rand` + `base64` for token generation, `reqwest` for the Resend HTTP API, and `axum` for the middleware extractor.

## Developer Top-of-Mind

- **Tokens are SHA-256 hashed before database storage**: raw tokens are only sent to the user via email; the database stores the hash. Comparison is always hash-to-hash.
- **Access vs refresh tokens carry a `token_type` claim**: the middleware uses `decode_access_token` which rejects refresh tokens, preventing token confusion attacks.
- **Access tokens expire in 15 minutes; refresh tokens in 7 days**. These values are hardcoded in `jwt.rs`.
- **`LoggingEmailSender`** is the default in development -- it prints the magic link to stdout/logs instead of sending real email. Swap to `ResendClient` in production via config.
- The `AuthUser` extractor reads the `JwtSecret` from Axum request extensions, which is injected by middleware in the API crate's router.

## Testing

```sh
cargo test -p finima-auth
```

Tests cover JWT encode/decode roundtrips, token type discrimination, magic link generation, and hash determinism. No external services required -- use `LoggingEmailSender` for tests.

## Authentication Flow

1. Client sends email to `POST /api/auth/magic-link`
2. Server generates a random token, SHA-256 hashes it, stores the hash in the database
3. Server sends the raw token via email (or logs it in dev mode)
4. User clicks the link, client sends the token to `POST /api/auth/verify`
5. Server hashes the received token, looks up the hash in the database
6. On match, server issues an access token (15 min) and refresh token (7 days)
7. Client uses the access token in `Authorization: Bearer <token>` headers
8. When the access token expires, client calls `POST /api/auth/refresh` with the refresh token

## Architecture Notes

This crate is intentionally thin -- it handles only token mechanics and email delivery. Session persistence (storing/revoking refresh tokens) lives in `finima-db::PgSessionRepo`. Route protection is handled by the `AuthUser` extractor, which any Axum handler can include in its signature.
