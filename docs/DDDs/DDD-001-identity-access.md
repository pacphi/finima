# DDD-001: Identity & Access Bounded Context

**Date:** 2026-04-10  
**Crate:** `finima-auth`

---

## 1. Purpose

Manages user identity, authentication lifecycle, and session management. This context owns the "who are you?" question and issues credentials consumed by all other contexts.

## 2. Ubiquitous Language

| Term              | Definition                                                                                                       |
| ----------------- | ---------------------------------------------------------------------------------------------------------------- |
| **User**          | A person with a verified email address who can access the system.                                                |
| **Magic Link**    | A time-limited, single-use URL sent via email that authenticates a user without a password.                      |
| **Token**         | A 32-byte cryptographically random value. The raw token is sent to the user; only the SHA-256 hash is stored.    |
| **Session**       | A pair of JWT access token (15 min) and refresh token (7 days) that proves the user's identity to the API.       |
| **Access Token**  | Short-lived JWT embedded in API request headers. Contains user ID and expiry claims.                             |
| **Refresh Token** | Longer-lived, single-use token used to obtain a new access token without re-authentication. Rotated on each use. |

## 3. Aggregates

### User (Aggregate Root)

```text
User
  id: UUID
  email: Email (unique, immutable after creation)
  display_name: String
  preferences: UserPreferences (JSONB)
  created_at: DateTime
  updated_at: DateTime
```

**Invariants:**

- Email must be valid and unique across the system.
- A user can only be created through the magic link verification flow (never directly).
- `preferences` defaults to `{}` on creation; individual keys are set during onboarding and settings.

### MagicLink (Entity, short-lived)

```text
MagicLink
  id: UUID
  email: Email
  token_hash: String (SHA-256 of raw token)
  expires_at: DateTime (created_at + 15 minutes)
  used_at: DateTime? (set on verification)
```

**Invariants:**

- A magic link can only be used once (`used_at` must be null).
- A magic link expires after 15 minutes.
- At most 5 magic links may be created per email per hour (rate limit, enforced at API layer).

### Session (Entity)

```text
Session
  id: UUID
  user_id: UUID (FK -> User)
  refresh_token_hash: String
  expires_at: DateTime (created_at + 7 days)
```

**Invariants:**

- Refresh tokens are single-use: consuming one invalidates it and creates a new session.
- Sessions can be revoked (logout) which deletes the record.

## 4. Domain Events

| Event                | Triggered By                                     | Consumed By                                           |
| -------------------- | ------------------------------------------------ | ----------------------------------------------------- |
| `MagicLinkRequested` | User submits email on sign-in page               | Email delivery service (Resend)                       |
| `UserAuthenticated`  | Magic link successfully verified                 | API layer (issue JWT), Onboarding (check if new user) |
| `UserCreated`        | First-time magic link verification for new email | Onboarding flow                                       |
| `SessionRefreshed`   | Client uses refresh token                        | API layer (issue new JWT)                             |
| `SessionRevoked`     | User logs out                                    | API layer (invalidate tokens)                         |

## 5. Services

### AuthService

- `request_magic_link(email) -> Result<()>` — Generate token, store hash, call Resend.
- `verify_magic_link(email, raw_token) -> Result<(User, AccessToken, RefreshToken)>` — Validate, create user if new, issue session.
- `refresh_session(refresh_token) -> Result<(AccessToken, RefreshToken)>` — Rotate refresh token, issue new access token.
- `revoke_session(session_id) -> Result<()>` — Logout.

### ResendClient (Infrastructure)

- `send_magic_link_email(email, link_url) -> Result<()>` — Calls Resend API to deliver the email.

## 6. Context Boundaries

**This context provides to other contexts:**

- `AuthUser` extractor (Axum middleware) — validates JWT and injects `UserId` into request handlers.
- User identity (ID, email, display_name) for ownership checks.

**This context does NOT know about:**

- Portfolios, accounts, transactions, budgets, or any financial domain concepts.
- The existence of the LLM or file parsers.

## 7. Anti-Corruption Layer

The Resend API is the only external dependency. The `ResendClient` wraps it behind a `trait EmailSender`, enabling a test double for development and test environments. **In production (`APP_ENV=production`), only the real `ResendClient` implementation is used.**

```rust
#[async_trait]
trait EmailSender: Send + Sync {
    async fn send_magic_link(&self, to: &str, link: &str) -> Result<()>;
}

// Production: ResendClient (real HTTP calls to Resend API)
// Dev/Test:   LoggingEmailSender (logs the link to stdout, never sends real email)
```

The implementation is selected based on the `APP_ENV` configuration profile. The `LoggingEmailSender` test double is excluded from production builds via `#[cfg(not(feature = "production"))]` or runtime config gating.
