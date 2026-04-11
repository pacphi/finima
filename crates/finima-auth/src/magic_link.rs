//! Magic link token generation, hashing, and URL construction.
//!
//! Tokens are 32 bytes of cryptographic randomness from `OsRng`, base64url-encoded.
//! Only the SHA-256 hash of the token is stored in the database; the raw token
//! is sent to the user via email and never persisted.

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use rand::rngs::OsRng;
use rand::RngCore;
use sha2::{Digest, Sha256};

/// Generate a cryptographically random magic link token.
///
/// Returns `(raw_token, token_hash)` where:
/// - `raw_token` is 32 bytes, base64url-encoded (no padding), suitable for inclusion in a URL.
/// - `token_hash` is the hex-encoded SHA-256 hash of the raw token string, for database storage.
pub fn generate_token() -> (String, String) {
    let mut bytes = [0u8; 32];
    OsRng.fill_bytes(&mut bytes);
    let raw_token = URL_SAFE_NO_PAD.encode(bytes);
    let token_hash = hash_token(&raw_token);
    (raw_token, token_hash)
}

/// Compute the SHA-256 hash of a raw token string.
///
/// The hash is returned as a lowercase hex string. This is deterministic:
/// the same input always produces the same output.
pub fn hash_token(raw_token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(raw_token.as_bytes());
    let result = hasher.finalize();
    hex::encode(result)
}

/// Build the full magic link verification URL.
///
/// The URL includes both the token and the email as query parameters so
/// the verification endpoint can look up the correct magic link record.
///
/// # Example
///
/// ```
/// use finima_auth::magic_link::build_magic_link_url;
///
/// let url = build_magic_link_url("https://app.finima.dev", "abc123", "user@example.com");
/// assert!(url.starts_with("https://app.finima.dev/auth/verify?token=abc123&email="));
/// ```
pub fn build_magic_link_url(base_url: &str, token: &str, email: &str) -> String {
    let encoded_email = urlencoding::encode(email);
    format!(
        "{}/auth/verify?token={}&email={}",
        base_url.trim_end_matches('/'),
        token,
        encoded_email
    )
}

// We need the hex crate for encoding; let's use sha2's output directly.
// Actually, we used hex::encode above — we need to add it or use a manual approach.
// We'll use a manual hex encoding to avoid an extra dependency.

mod hex {
    /// Encode bytes as a lowercase hex string.
    pub fn encode(bytes: impl AsRef<[u8]>) -> String {
        bytes
            .as_ref()
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_token_produces_valid_base64url() {
        let (raw_token, _hash) = generate_token();
        // 32 bytes base64url-encoded without padding = 43 characters
        assert_eq!(raw_token.len(), 43);
        // Verify it decodes back to 32 bytes
        let decoded = URL_SAFE_NO_PAD.decode(&raw_token).expect("valid base64url");
        assert_eq!(decoded.len(), 32);
    }

    #[test]
    fn generate_token_produces_unique_tokens() {
        let (token1, _) = generate_token();
        let (token2, _) = generate_token();
        assert_ne!(token1, token2);
    }

    #[test]
    fn hash_token_is_deterministic() {
        let input = "test-token-value";
        let hash1 = hash_token(input);
        let hash2 = hash_token(input);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn hash_token_produces_valid_sha256_hex() {
        let hash = hash_token("hello");
        // SHA-256 hex output is always 64 characters
        assert_eq!(hash.len(), 64);
        // All characters should be valid hex
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn hash_token_different_inputs_produce_different_hashes() {
        let hash1 = hash_token("token-a");
        let hash2 = hash_token("token-b");
        assert_ne!(hash1, hash2);
    }

    #[test]
    fn build_magic_link_url_correct_format() {
        let url = build_magic_link_url("https://app.finima.dev", "mytoken123", "user@example.com");
        assert_eq!(
            url,
            "https://app.finima.dev/auth/verify?token=mytoken123&email=user%40example.com"
        );
    }

    #[test]
    fn build_magic_link_url_strips_trailing_slash() {
        let url = build_magic_link_url("https://app.finima.dev/", "tok", "a@b.com");
        assert!(url.starts_with("https://app.finima.dev/auth/verify?token=tok&email="));
    }

    #[test]
    fn generate_token_hash_matches_manual_hash() {
        let (raw_token, token_hash) = generate_token();
        let manual_hash = hash_token(&raw_token);
        assert_eq!(token_hash, manual_hash);
    }
}
