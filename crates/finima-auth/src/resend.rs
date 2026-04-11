//! Email delivery for magic link authentication.
//!
//! Provides the `EmailSender` trait and two implementations:
//! - `ResendClient`: sends real emails via the Resend API (production use).
//! - `LoggingEmailSender`: logs the magic link URL instead of sending email (dev/test only).

use async_trait::async_trait;
use serde::Serialize;

use crate::AuthError;

/// Trait for sending magic link emails.
///
/// This abstraction allows swapping between real email delivery (Resend API)
/// and a test double that simply logs the link.
#[async_trait]
pub trait EmailSender: Send + Sync {
    /// Send a magic link email to the specified address.
    ///
    /// # Arguments
    /// * `to` - The recipient email address.
    /// * `link_url` - The full magic link verification URL.
    async fn send_magic_link(&self, to: &str, link_url: &str) -> Result<(), AuthError>;
}

/// Resend API client for sending magic link emails in production.
///
/// Uses the Resend REST API to deliver branded authentication emails.
/// Requires a valid API key from <https://resend.com>.
pub struct ResendClient {
    api_key: String,
    from_email: String,
    http_client: reqwest::Client,
}

#[derive(Serialize)]
struct SendEmailRequest<'a> {
    from: &'a str,
    to: &'a str,
    subject: &'a str,
    html: String,
}

impl ResendClient {
    /// Create a new Resend client.
    ///
    /// # Arguments
    /// * `api_key` - Resend API key (starts with `re_`).
    /// * `from_email` - Sender address, e.g. `"Finima <auth@finima.dev>"`.
    pub fn new(api_key: String, from_email: String) -> Self {
        Self {
            api_key,
            from_email,
            http_client: reqwest::Client::new(),
        }
    }

    /// Send a magic link email via the Resend API.
    ///
    /// POSTs to `https://api.resend.com/emails` with a JSON body containing
    /// the sender, recipient, subject, and HTML content.
    pub async fn send_magic_link_email(
        &self,
        to: &str,
        magic_link_url: &str,
    ) -> Result<(), AuthError> {
        let html_body = format!(
            r#"<!DOCTYPE html>
<html>
<body style="font-family: sans-serif; max-width: 600px; margin: 0 auto; padding: 20px;">
  <h2>Sign in to Finima</h2>
  <p>Click the button below to sign in to your account. This link expires in 15 minutes.</p>
  <a href="{url}" style="display: inline-block; background: #4F46E5; color: white; padding: 12px 24px; text-decoration: none; border-radius: 6px; margin: 16px 0;">
    Sign In
  </a>
  <p style="color: #666; font-size: 14px;">
    If you didn't request this email, you can safely ignore it.
  </p>
  <p style="color: #999; font-size: 12px;">
    Or copy this link: {url}
  </p>
</body>
</html>"#,
            url = magic_link_url,
        );

        let request = SendEmailRequest {
            from: &self.from_email,
            to,
            subject: "Sign in to Finima",
            html: html_body,
        };

        let response = self
            .http_client
            .post("https://api.resend.com/emails")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&request)
            .send()
            .await
            .map_err(|e| AuthError::EmailDelivery(e.to_string()))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "unknown error".to_string());
            return Err(AuthError::EmailDelivery(format!(
                "Resend API returned {}: {}",
                status, body
            )));
        }

        tracing::info!(to = to, "Magic link email sent successfully via Resend");
        Ok(())
    }
}

#[async_trait]
impl EmailSender for ResendClient {
    async fn send_magic_link(&self, to: &str, link_url: &str) -> Result<(), AuthError> {
        self.send_magic_link_email(to, link_url).await
    }
}

/// Development/test email sender that logs the magic link instead of sending real email.
///
/// **WARNING: This is for development and testing only.** It prints the magic link URL
/// to the application log so developers can click it directly. Never use this in production
/// as it exposes authentication tokens in logs.
///
/// # Example log output
///
/// ```text
/// INFO finima_auth::resend: [DEV] Magic link for user@example.com: https://localhost:3000/auth/verify?token=abc...
/// ```
pub struct LoggingEmailSender;

#[async_trait]
impl EmailSender for LoggingEmailSender {
    async fn send_magic_link(&self, to: &str, link_url: &str) -> Result<(), AuthError> {
        tracing::info!(
            to = to,
            link = link_url,
            "[DEV] Magic link for {}: {}",
            to,
            link_url
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn logging_email_sender_succeeds() {
        let sender = LoggingEmailSender;
        let result = sender
            .send_magic_link("test@example.com", "https://example.com/verify?token=abc")
            .await;
        assert!(result.is_ok());
    }

    #[test]
    fn resend_client_construction() {
        let client = ResendClient::new(
            "re_test_key".to_string(),
            "Finima <auth@finima.dev>".to_string(),
        );
        assert_eq!(client.api_key, "re_test_key");
        assert_eq!(client.from_email, "Finima <auth@finima.dev>");
    }
}
