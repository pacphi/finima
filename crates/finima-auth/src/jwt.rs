//! JWT encoding and decoding for access and refresh tokens.
//!
//! Access tokens are short-lived (15 minutes) and contain user identity claims.
//! Refresh tokens are longer-lived (7 days) and are used to obtain new access tokens.

use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::AuthError;

/// The type of JWT token, used to prevent token confusion attacks.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum TokenType {
    Access,
    Refresh,
}

/// JWT claims embedded in access and refresh tokens.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Claims {
    /// Subject: the user's UUID as a string.
    pub sub: String,
    /// The user's email address. Present in access tokens; empty in refresh tokens.
    pub email: String,
    /// Expiry timestamp (seconds since Unix epoch).
    pub exp: usize,
    /// Issued-at timestamp (seconds since Unix epoch).
    pub iat: usize,
    /// Token type discriminator to prevent using refresh tokens as access tokens
    /// and vice versa.
    pub token_type: TokenType,
}

/// Encode a JWT access token with a 15-minute expiry.
///
/// The token contains the user's ID and email in its claims and is signed
/// with HMAC-SHA256 using the provided secret.
pub fn encode_access_token(user_id: Uuid, email: &str, secret: &str) -> Result<String, AuthError> {
    let now = chrono::Utc::now().timestamp() as usize;
    let claims = Claims {
        sub: user_id.to_string(),
        email: email.to_string(),
        exp: now + 15 * 60, // 15 minutes
        iat: now,
        token_type: TokenType::Access,
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| AuthError::TokenEncoding(e.to_string()))
}

/// Encode a JWT refresh token with a 7-day expiry.
///
/// Refresh tokens carry only the user ID (no email) since they are used
/// solely to obtain new access tokens.
pub fn encode_refresh_token(user_id: Uuid, secret: &str) -> Result<String, AuthError> {
    let now = chrono::Utc::now().timestamp() as usize;
    let claims = Claims {
        sub: user_id.to_string(),
        email: String::new(),
        exp: now + 7 * 24 * 60 * 60, // 7 days
        iat: now,
        token_type: TokenType::Refresh,
    };
    encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
    .map_err(|e| AuthError::TokenEncoding(e.to_string()))
}

/// Decode and validate a JWT token.
///
/// Verifies the signature and checks that the token has not expired.
/// Returns the embedded claims on success. Does **not** check `token_type`;
/// use [`decode_access_token`] or [`decode_refresh_token`] for type-safe
/// decoding.
pub fn decode_token(token: &str, secret: &str) -> Result<Claims, AuthError> {
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )
    .map_err(|e| AuthError::TokenDecoding(e.to_string()))?;
    Ok(token_data.claims)
}

/// Decode a JWT and verify it is an **access** token.
pub fn decode_access_token(token: &str, secret: &str) -> Result<Claims, AuthError> {
    let claims = decode_token(token, secret)?;
    if claims.token_type != TokenType::Access {
        return Err(AuthError::TokenDecoding(
            "expected access token".to_string(),
        ));
    }
    Ok(claims)
}

/// Decode a JWT and verify it is a **refresh** token.
pub fn decode_refresh_token(token: &str, secret: &str) -> Result<Claims, AuthError> {
    let claims = decode_token(token, secret)?;
    if claims.token_type != TokenType::Refresh {
        return Err(AuthError::TokenDecoding(
            "expected refresh token".to_string(),
        ));
    }
    Ok(claims)
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_SECRET: &str = "test-secret-key-for-jwt-signing-only";

    #[test]
    fn access_token_roundtrip_preserves_claims() {
        let user_id = Uuid::new_v4();
        let email = "test@example.com";

        let token = encode_access_token(user_id, email, TEST_SECRET).unwrap();
        let claims = decode_token(&token, TEST_SECRET).unwrap();

        assert_eq!(claims.sub, user_id.to_string());
        assert_eq!(claims.email, email);
        assert_eq!(claims.token_type, TokenType::Access);
        // iat should be recent (within last 5 seconds)
        let now = chrono::Utc::now().timestamp() as usize;
        assert!(claims.iat <= now && claims.iat >= now - 5);
        // exp should be ~15 minutes from now
        assert!(claims.exp > now && claims.exp <= now + 15 * 60 + 5);
    }

    #[test]
    fn refresh_token_roundtrip_preserves_user_id() {
        let user_id = Uuid::new_v4();

        let token = encode_refresh_token(user_id, TEST_SECRET).unwrap();
        let claims = decode_token(&token, TEST_SECRET).unwrap();

        assert_eq!(claims.sub, user_id.to_string());
        assert!(claims.email.is_empty());
        assert_eq!(claims.token_type, TokenType::Refresh);
        // exp should be ~7 days from now
        let now = chrono::Utc::now().timestamp() as usize;
        assert!(claims.exp > now + 6 * 24 * 60 * 60);
    }

    #[test]
    fn decode_access_token_rejects_refresh_token() {
        let user_id = Uuid::new_v4();
        let token = encode_refresh_token(user_id, TEST_SECRET).unwrap();
        let result = decode_access_token(&token, TEST_SECRET);
        assert!(result.is_err());
    }

    #[test]
    fn decode_refresh_token_rejects_access_token() {
        let user_id = Uuid::new_v4();
        let token = encode_access_token(user_id, "a@b.com", TEST_SECRET).unwrap();
        let result = decode_refresh_token(&token, TEST_SECRET);
        assert!(result.is_err());
    }

    #[test]
    fn decode_with_wrong_secret_fails() {
        let user_id = Uuid::new_v4();
        let token = encode_access_token(user_id, "a@b.com", TEST_SECRET).unwrap();

        let result = decode_token(&token, "wrong-secret");
        assert!(result.is_err());
    }

    #[test]
    fn decode_expired_token_fails() {
        // Manually create a token that expired 1 hour ago
        let now = chrono::Utc::now().timestamp() as usize;
        let claims = Claims {
            sub: Uuid::new_v4().to_string(),
            email: "expired@example.com".to_string(),
            exp: now - 3600, // expired 1 hour ago
            iat: now - 7200, // issued 2 hours ago
            token_type: TokenType::Access,
        };
        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(TEST_SECRET.as_bytes()),
        )
        .unwrap();

        let result = decode_token(&token, TEST_SECRET);
        assert!(result.is_err());
    }

    #[test]
    fn decode_malformed_token_fails() {
        let result = decode_token("not-a-valid-jwt", TEST_SECRET);
        assert!(result.is_err());
    }
}
