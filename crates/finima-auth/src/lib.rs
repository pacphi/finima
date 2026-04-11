//! Authentication system for Finima.
//!
//! This crate implements passwordless magic link authentication as described
//! in ADR-002. It provides:
//!
//! - **Magic link tokens**: Cryptographic token generation and SHA-256 hashing.
//! - **JWT encoding/decoding**: Short-lived access tokens and longer-lived refresh tokens.
//! - **Email delivery**: Resend API client and a dev/test logging double.
//! - **Axum middleware**: `AuthUser` extractor for protecting routes.

pub mod jwt;
pub mod magic_link;
pub mod middleware;
pub mod resend;

// Re-export key public types for convenient access.
pub use jwt::{Claims, TokenType};
pub use magic_link::{build_magic_link_url, generate_token, hash_token};
pub use middleware::{AuthRejection, AuthUser, JwtSecret};
pub use resend::{EmailSender, LoggingEmailSender, ResendClient};

use thiserror::Error;

/// Errors that can occur during authentication operations.
#[derive(Debug, Error)]
pub enum AuthError {
    /// Failed to encode a JWT.
    #[error("Token encoding failed: {0}")]
    TokenEncoding(String),

    /// Failed to decode or validate a JWT.
    #[error("Token decoding failed: {0}")]
    TokenDecoding(String),

    /// Email delivery failed.
    #[error("Email delivery failed: {0}")]
    EmailDelivery(String),

    /// The magic link is invalid, expired, or already used.
    #[error("Invalid magic link: {0}")]
    InvalidMagicLink(String),
}

impl From<AuthError> for finima_core::AppError {
    fn from(err: AuthError) -> Self {
        match err {
            AuthError::TokenDecoding(_) | AuthError::InvalidMagicLink(_) => {
                finima_core::AppError::Unauthorized
            }
            AuthError::TokenEncoding(msg) => finima_core::AppError::InternalError(msg),
            AuthError::EmailDelivery(msg) => finima_core::AppError::InternalError(msg),
        }
    }
}
