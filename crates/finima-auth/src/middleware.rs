//! Axum middleware for JWT-based authentication.
//!
//! Provides the `AuthUser` extractor which can be used in Axum handler
//! function signatures to require authentication. It extracts the Bearer
//! token from the `Authorization` header, decodes and validates the JWT,
//! and provides the authenticated user's identity.

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use uuid::Uuid;

use crate::jwt;

/// Authenticated user identity extracted from a valid JWT.
///
/// Use this as an extractor in Axum handlers to require authentication:
///
/// ```rust,ignore
/// async fn my_handler(user: AuthUser) -> impl IntoResponse {
///     format!("Hello, user {}", user.user_id)
/// }
/// ```
#[derive(Debug, Clone)]
pub struct AuthUser {
    /// The authenticated user's UUID.
    pub user_id: Uuid,
    /// The authenticated user's email address.
    pub email: String,
}

/// Error type returned when authentication fails in the middleware.
#[derive(Debug)]
pub enum AuthRejection {
    /// No Authorization header was provided.
    MissingToken,
    /// The Authorization header format is invalid (not "Bearer <token>").
    InvalidFormat,
    /// The JWT is invalid, expired, or has a bad signature.
    InvalidToken(String),
    /// The `sub` claim is not a valid UUID.
    InvalidUserId,
}

impl IntoResponse for AuthRejection {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AuthRejection::MissingToken => {
                (StatusCode::UNAUTHORIZED, "Missing authorization token")
            }
            AuthRejection::InvalidFormat => (
                StatusCode::UNAUTHORIZED,
                "Invalid authorization header format",
            ),
            AuthRejection::InvalidToken(_) => {
                (StatusCode::UNAUTHORIZED, "Invalid or expired token")
            }
            AuthRejection::InvalidUserId => {
                (StatusCode::UNAUTHORIZED, "Invalid user identity in token")
            }
        };
        (status, message).into_response()
    }
}

/// The key used to look up the JWT secret from Axum's state/extensions.
///
/// Handlers or middleware layers should insert this into the request extensions
/// or use a shared state type. For simplicity, this extractor reads the secret
/// from the `JwtSecret` extension on the request.
#[derive(Debug, Clone)]
pub struct JwtSecret(pub String);

impl<S> FromRequestParts<S> for AuthUser
where
    S: Send + Sync,
{
    type Rejection = AuthRejection;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        // Get JWT secret from request extensions
        let secret = parts
            .extensions
            .get::<JwtSecret>()
            .ok_or(AuthRejection::InvalidToken(
                "JWT secret not configured".to_string(),
            ))?;

        // Extract Bearer token from Authorization header
        let auth_header = parts
            .headers
            .get(http::header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .ok_or(AuthRejection::MissingToken)?;

        let token = auth_header
            .strip_prefix("Bearer ")
            .ok_or(AuthRejection::InvalidFormat)?;

        if token.is_empty() {
            return Err(AuthRejection::InvalidFormat);
        }

        // Decode and validate the JWT — must be an access token
        let claims = jwt::decode_access_token(token, &secret.0)
            .map_err(|e| AuthRejection::InvalidToken(e.to_string()))?;

        // Parse the user ID from the sub claim
        let user_id = Uuid::parse_str(&claims.sub).map_err(|_| AuthRejection::InvalidUserId)?;

        Ok(AuthUser {
            user_id,
            email: claims.email,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;

    const TEST_SECRET: &str = "test-secret-key-for-jwt-signing-only";

    /// Helper to build a request with optional auth header and JWT secret extension.
    fn build_request(auth_header: Option<&str>, include_secret: bool) -> Request<Body> {
        let mut builder = Request::builder().uri("/test");
        if let Some(header) = auth_header {
            builder = builder.header("Authorization", header);
        }
        let mut req = builder.body(Body::empty()).unwrap();
        if include_secret {
            req.extensions_mut()
                .insert(JwtSecret(TEST_SECRET.to_string()));
        }
        req
    }

    async fn extract_auth_user(request: Request<Body>) -> Result<AuthUser, AuthRejection> {
        let (mut parts, _body) = request.into_parts();
        AuthUser::from_request_parts(&mut parts, &()).await
    }

    #[tokio::test]
    async fn valid_token_extracts_auth_user() {
        let user_id = Uuid::new_v4();
        let email = "test@example.com";
        let token = jwt::encode_access_token(user_id, email, TEST_SECRET).unwrap();

        let request = build_request(Some(&format!("Bearer {}", token)), true);
        let result = extract_auth_user(request).await;

        let auth_user = result.expect("should extract auth user");
        assert_eq!(auth_user.user_id, user_id);
        assert_eq!(auth_user.email, email);
    }

    #[tokio::test]
    async fn missing_auth_header_returns_401() {
        let request = build_request(None, true);
        let result = extract_auth_user(request).await;
        assert!(matches!(result, Err(AuthRejection::MissingToken)));
    }

    #[tokio::test]
    async fn invalid_format_returns_401() {
        let request = build_request(Some("Basic abc123"), true);
        let result = extract_auth_user(request).await;
        assert!(matches!(result, Err(AuthRejection::InvalidFormat)));
    }

    #[tokio::test]
    async fn expired_token_returns_401() {
        // Create an expired token manually
        use jsonwebtoken::{encode, EncodingKey, Header};
        let now = chrono::Utc::now().timestamp() as usize;
        let claims = jwt::Claims {
            sub: Uuid::new_v4().to_string(),
            email: "expired@example.com".to_string(),
            exp: now - 3600,
            iat: now - 7200,
            token_type: jwt::TokenType::Access,
        };
        let token = encode(
            &Header::default(),
            &claims,
            &EncodingKey::from_secret(TEST_SECRET.as_bytes()),
        )
        .unwrap();

        let request = build_request(Some(&format!("Bearer {}", token)), true);
        let result = extract_auth_user(request).await;
        assert!(matches!(result, Err(AuthRejection::InvalidToken(_))));
    }

    #[tokio::test]
    async fn invalid_jwt_returns_401() {
        let request = build_request(Some("Bearer not.a.valid.jwt"), true);
        let result = extract_auth_user(request).await;
        assert!(matches!(result, Err(AuthRejection::InvalidToken(_))));
    }

    #[tokio::test]
    async fn empty_bearer_token_returns_401() {
        let request = build_request(Some("Bearer "), true);
        let result = extract_auth_user(request).await;
        assert!(matches!(result, Err(AuthRejection::InvalidFormat)));
    }
}
