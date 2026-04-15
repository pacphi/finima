pub mod fuzzy;
pub mod mcc_loader;
pub mod merchant_db;

pub use merchant_db::MerchantRegistry;

use crate::types::CategoryAssignment;

/// Trait for Tier 0 merchant-based categorization.
pub trait MerchantLookup: Send + Sync {
    /// Attempt to categorize a transaction by merchant name and optional MCC code.
    fn lookup(&self, description: &str, mcc: Option<u16>) -> Option<CategoryAssignment>;
}

impl MerchantLookup for MerchantRegistry {
    fn lookup(&self, description: &str, mcc: Option<u16>) -> Option<CategoryAssignment> {
        self.lookup(description, mcc)
    }
}
