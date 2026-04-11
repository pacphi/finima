# ADR-002: Passwordless Authentication via Magic Links

**Status:** Accepted  
**Date:** 2026-04-10  
**Deciders:** Chris Phillipson

---

## Context

Finima needs user authentication for a self-hosted personal finance app. The target audience includes privacy advocates and non-technical household members. Password management adds friction (forgotten passwords, password reuse, storage complexity). OAuth providers (Google, GitHub) introduce third-party dependencies that conflict with the privacy-first mission.

## Decision

Implement **passwordless email magic links** using the Resend API for email delivery.

**Flow:**

1. User enters email address.
2. Backend generates 32 bytes of cryptographic randomness (`OsRng`), base64url-encodes it as the raw token.
3. Backend stores `SHA-256(token)` with the email and a 15-minute expiry in a `magic_links` table. The raw token is never persisted.
4. Backend calls Resend API to send a branded email with a verification link containing the raw token.
5. User clicks the link. Backend hashes the provided token, looks up the match, validates expiry and single-use.
6. On success, backend issues a JWT access token (15 min TTL) and a refresh token (7 days, single-use rotation).

**Token security properties:**

- 256 bits of entropy (32 bytes) — brute force infeasible.
- SHA-256 hashed before storage — database breach does not expose valid tokens.
- Single-use — marked `used_at` immediately on verification.
- Time-limited — 15-minute window balances usability and security.

## Consequences

**Positive:**

- No password storage, no bcrypt/argon2 cost, no password reset flow.
- Users cannot reuse weak passwords across services.
- Onboarding reduced to "enter email, click link."
- Works naturally for household members who may not be technically sophisticated.

**Negative:**

- Depends on Resend (external email API) for the auth flow. Mitigated: Resend is used only for sending; verification is entirely local.
- Email delivery latency (typically <5s via Resend) adds friction compared to password login.
- Users without email access (e.g., shared device, lost email) cannot sign in. Mitigated: this is an acceptable constraint for a personal finance app where email is a universal assumption.
- Rate limiting is critical to prevent magic link abuse (5 requests per IP address per minute).

## Alternatives Considered

1. **Password-based auth (bcrypt/argon2)** — Adds password UX complexity, storage burden, and reset flows. Rejected for a privacy-focused app where simplicity matters.
2. **OAuth (Google/GitHub)** — Introduces third-party identity dependency, contradicts privacy-first philosophy. Rejected.
3. **WebAuthn/Passkeys** — Excellent security but requires device-specific credential setup. Too complex for household members and diverse device access patterns. Deferred to a future enhancement.
4. **TOTP/Authenticator app** — Adds setup friction. Could be a future 2FA layer on top of magic links.
