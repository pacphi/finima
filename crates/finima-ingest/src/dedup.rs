//! Duplicate detection via SHA-256 hashing.

use chrono::NaiveDate;
use rust_decimal::Decimal;
use sha2::{Digest, Sha256};

/// Compute a deterministic dedup hash for a transaction.
///
/// The hash is `SHA-256(date || amount || description)` encoded as a lowercase hex string.
/// Same inputs always produce the same hash.
pub fn compute_dedup_hash(date: &NaiveDate, amount: &Decimal, description: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(date.format("%Y-%m-%d").to_string().as_bytes());
    hasher.update(b"||");
    hasher.update(amount.to_string().as_bytes());
    hasher.update(b"||");
    hasher.update(description.as_bytes());
    let result = hasher.finalize();
    // Format as lowercase hex
    result.iter().map(|b| format!("{:02x}", b)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_decimal_macros::dec;

    #[test]
    fn dedup_hash_deterministic() {
        let date = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
        let amount = dec!(-45.99);
        let desc = "Grocery Store";

        let hash1 = compute_dedup_hash(&date, &amount, desc);
        let hash2 = compute_dedup_hash(&date, &amount, desc);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn dedup_hash_is_sha256_hex() {
        let date = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
        let amount = dec!(-45.99);
        let hash = compute_dedup_hash(&date, &amount, "Test");
        // SHA-256 hex = 64 chars
        assert_eq!(hash.len(), 64);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn dedup_hash_different_dates() {
        let amount = dec!(-45.99);
        let desc = "Grocery Store";
        let h1 = compute_dedup_hash(
            &NaiveDate::from_ymd_opt(2024, 1, 15).unwrap(),
            &amount,
            desc,
        );
        let h2 = compute_dedup_hash(
            &NaiveDate::from_ymd_opt(2024, 1, 16).unwrap(),
            &amount,
            desc,
        );
        assert_ne!(h1, h2);
    }

    #[test]
    fn dedup_hash_different_amounts() {
        let date = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
        let desc = "Grocery Store";
        let h1 = compute_dedup_hash(&date, &dec!(-45.99), desc);
        let h2 = compute_dedup_hash(&date, &dec!(-46.00), desc);
        assert_ne!(h1, h2);
    }

    #[test]
    fn dedup_hash_different_descriptions() {
        let date = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();
        let amount = dec!(-45.99);
        let h1 = compute_dedup_hash(&date, &amount, "Grocery Store");
        let h2 = compute_dedup_hash(&date, &amount, "Coffee Shop");
        assert_ne!(h1, h2);
    }
}
