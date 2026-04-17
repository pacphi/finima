use std::fmt;
use std::str::FromStr;

use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};

/// All supported account types in Finima.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text")]
#[sqlx(rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum AccountType {
    Checking,
    Savings,
    CreditCard,
    InvestmentBrokerage,
    InvestmentRetirement,
    LoanMortgage,
    LoanAuto,
    LoanStudent,
    LoanPersonal,
    Cash,
    Crypto,
    Other,
}

impl fmt::Display for AccountType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Checking => "checking",
            Self::Savings => "savings",
            Self::CreditCard => "credit_card",
            Self::InvestmentBrokerage => "investment_brokerage",
            Self::InvestmentRetirement => "investment_retirement",
            Self::LoanMortgage => "loan_mortgage",
            Self::LoanAuto => "loan_auto",
            Self::LoanStudent => "loan_student",
            Self::LoanPersonal => "loan_personal",
            Self::Cash => "cash",
            Self::Crypto => "crypto",
            Self::Other => "other",
        };
        write!(f, "{}", s)
    }
}

impl FromStr for AccountType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "checking" => Ok(Self::Checking),
            "savings" => Ok(Self::Savings),
            "credit_card" => Ok(Self::CreditCard),
            "investment_brokerage" => Ok(Self::InvestmentBrokerage),
            "investment_retirement" => Ok(Self::InvestmentRetirement),
            "loan_mortgage" => Ok(Self::LoanMortgage),
            "loan_auto" => Ok(Self::LoanAuto),
            "loan_student" => Ok(Self::LoanStudent),
            "loan_personal" => Ok(Self::LoanPersonal),
            "cash" => Ok(Self::Cash),
            "crypto" => Ok(Self::Crypto),
            "other" => Ok(Self::Other),
            _ => Err(format!("Unknown account type: {}", s)),
        }
    }
}

/// Canonical direction of a transaction relative to its account.
///
/// Computed at import time by the `SignNormalizer` service (see ADR-018).
/// Downstream consumers (Sankey, reports, queries) should branch on this
/// field rather than the sign of `amount`, which varies by institution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text")]
#[sqlx(rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum TransactionDirection {
    /// Money entering the account (deposits, payments received, paycheck credits).
    Inflow,
    /// Money leaving the account (purchases, debit card spending, charges, transfers out).
    Outflow,
}

impl fmt::Display for TransactionDirection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Inflow => "inflow",
            Self::Outflow => "outflow",
        };
        write!(f, "{}", s)
    }
}

impl FromStr for TransactionDirection {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "inflow" => Ok(Self::Inflow),
            "outflow" => Ok(Self::Outflow),
            _ => Err(format!("Unknown transaction direction: {}", s)),
        }
    }
}

/// Whether an account represents an asset (positive balance = wealth)
/// or a liability (negative balance = debt under the canonical-amount
/// convention; see ADR-018).
///
/// Derived purely from `AccountType`; not stored in the database.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountRole {
    /// Checking, savings, cash, investment accounts. Balance up = wealth up.
    Asset,
    /// Credit cards, loans. Under canonical amounts, balance down = debt up:
    /// a negative balance on a credit card represents $X owed to the issuer,
    /// a positive balance represents a credit (overpayment) sitting on the
    /// card that behaves like cash.
    Liability,
}

impl AccountRole {
    /// Domain rule: asset vs liability is fully determined by account type.
    pub fn for_account_type(ty: AccountType) -> Self {
        match ty {
            AccountType::Checking
            | AccountType::Savings
            | AccountType::Cash
            | AccountType::Crypto
            | AccountType::InvestmentBrokerage
            | AccountType::InvestmentRetirement => Self::Asset,
            AccountType::CreditCard
            | AccountType::LoanMortgage
            | AccountType::LoanAuto
            | AccountType::LoanStudent
            | AccountType::LoanPersonal => Self::Liability,
            // "Other" is ambiguous; default to Asset for additive treatment.
            // Users with unusual needs can model via custom subtypes later.
            AccountType::Other => Self::Asset,
        }
    }

    /// Convenience predicate for call sites that only care whether an
    /// account is a liability (credit card, loan).
    pub fn is_liability_type(ty: AccountType) -> bool {
        matches!(Self::for_account_type(ty), Self::Liability)
    }

    /// Split a canonical-amount signed balance into its contributions
    /// to `(assets, liabilities)` for net-worth reporting. Encodes the
    /// ADR-018 domain rule in one place so dashboard / net-worth /
    /// per-portfolio summary handlers cannot drift:
    ///
    /// - **Asset account**: the entire balance counts toward assets,
    ///   including negative balances (overdrafts show up as reduced
    ///   assets, never as liabilities).
    /// - **Liability with negative balance**: real debt; the absolute
    ///   value contributes to liabilities.
    /// - **Liability with positive balance**: a credit on the card
    ///   (you've overpaid or received a refund) — cash-like, counts
    ///   toward assets, never liabilities.
    ///
    /// Returns `(asset_contribution, liability_contribution)`.
    pub fn classify_balance(account_type: AccountType, balance: Decimal) -> (Decimal, Decimal) {
        match Self::for_account_type(account_type) {
            Self::Asset => (balance, Decimal::ZERO),
            Self::Liability => {
                if balance.is_sign_negative() {
                    (Decimal::ZERO, -balance)
                } else {
                    (balance, Decimal::ZERO)
                }
            }
        }
    }
}

/// Frequency of a recurring transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text")]
#[sqlx(rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum Frequency {
    Daily,
    Weekly,
    Biweekly,
    Monthly,
    Quarterly,
    Semiannual,
    Annual,
    Variable,
}

impl fmt::Display for Frequency {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Daily => "daily",
            Self::Weekly => "weekly",
            Self::Biweekly => "biweekly",
            Self::Monthly => "monthly",
            Self::Quarterly => "quarterly",
            Self::Semiannual => "semiannual",
            Self::Annual => "annual",
            Self::Variable => "variable",
        };
        write!(f, "{}", s)
    }
}

impl FromStr for Frequency {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "daily" => Ok(Self::Daily),
            "weekly" => Ok(Self::Weekly),
            "biweekly" => Ok(Self::Biweekly),
            "monthly" => Ok(Self::Monthly),
            "quarterly" => Ok(Self::Quarterly),
            "semiannual" => Ok(Self::Semiannual),
            "annual" => Ok(Self::Annual),
            "variable" => Ok(Self::Variable),
            _ => Err(format!("Unknown frequency: {}", s)),
        }
    }
}

/// Status of a file upload/import job.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text")]
#[sqlx(rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum UploadStatus {
    Pending,
    Processing,
    Categorizing,
    Complete,
    Error,
}

impl fmt::Display for UploadStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Pending => "pending",
            Self::Processing => "processing",
            Self::Categorizing => "categorizing",
            Self::Complete => "complete",
            Self::Error => "error",
        };
        write!(f, "{}", s)
    }
}

impl FromStr for UploadStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(Self::Pending),
            "processing" => Ok(Self::Processing),
            "categorizing" => Ok(Self::Categorizing),
            "complete" => Ok(Self::Complete),
            "error" => Ok(Self::Error),
            _ => Err(format!("Unknown upload status: {}", s)),
        }
    }
}

/// Supported file formats for transaction import.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "text")]
#[sqlx(rename_all = "snake_case")]
#[serde(rename_all = "snake_case")]
pub enum FileFormat {
    Csv,
    Tsv,
    Ofx,
    Qfx,
    Qbo,
    Qif,
    Xls,
    Xlsx,
}

impl fmt::Display for FileFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Csv => "csv",
            Self::Tsv => "tsv",
            Self::Ofx => "ofx",
            Self::Qfx => "qfx",
            Self::Qbo => "qbo",
            Self::Qif => "qif",
            Self::Xls => "xls",
            Self::Xlsx => "xlsx",
        };
        write!(f, "{}", s)
    }
}

impl FromStr for FileFormat {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "csv" => Ok(Self::Csv),
            "tsv" => Ok(Self::Tsv),
            "ofx" => Ok(Self::Ofx),
            "qfx" => Ok(Self::Qfx),
            "qbo" => Ok(Self::Qbo),
            "qif" => Ok(Self::Qif),
            "xls" => Ok(Self::Xls),
            "xlsx" => Ok(Self::Xlsx),
            _ => Err(format!("Unknown file format: {}", s)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn account_type_roundtrip() {
        let variants = [
            AccountType::Checking,
            AccountType::Savings,
            AccountType::CreditCard,
            AccountType::InvestmentBrokerage,
            AccountType::InvestmentRetirement,
            AccountType::LoanMortgage,
            AccountType::LoanAuto,
            AccountType::LoanStudent,
            AccountType::LoanPersonal,
            AccountType::Cash,
            AccountType::Crypto,
            AccountType::Other,
        ];
        for variant in &variants {
            let s = variant.to_string();
            let parsed: AccountType = s.parse().unwrap();
            assert_eq!(*variant, parsed);
        }
    }

    #[test]
    fn account_type_invalid_str() {
        let result = "invalid_type".parse::<AccountType>();
        assert!(result.is_err());
    }

    #[test]
    fn frequency_roundtrip() {
        let variants = [
            Frequency::Daily,
            Frequency::Weekly,
            Frequency::Biweekly,
            Frequency::Monthly,
            Frequency::Quarterly,
            Frequency::Semiannual,
            Frequency::Annual,
            Frequency::Variable,
        ];
        for variant in &variants {
            let s = variant.to_string();
            let parsed: Frequency = s.parse().unwrap();
            assert_eq!(*variant, parsed);
        }
    }

    #[test]
    fn upload_status_roundtrip() {
        let variants = [
            UploadStatus::Pending,
            UploadStatus::Processing,
            UploadStatus::Categorizing,
            UploadStatus::Complete,
            UploadStatus::Error,
        ];
        for variant in &variants {
            let s = variant.to_string();
            let parsed: UploadStatus = s.parse().unwrap();
            assert_eq!(*variant, parsed);
        }
    }

    #[test]
    fn file_format_roundtrip() {
        let variants = [
            FileFormat::Csv,
            FileFormat::Tsv,
            FileFormat::Ofx,
            FileFormat::Qfx,
            FileFormat::Qbo,
            FileFormat::Qif,
            FileFormat::Xls,
            FileFormat::Xlsx,
        ];
        for variant in &variants {
            let s = variant.to_string();
            let parsed: FileFormat = s.parse().unwrap();
            assert_eq!(*variant, parsed);
        }
    }

    #[test]
    fn account_type_serde_roundtrip() {
        let at = AccountType::CreditCard;
        let json = serde_json::to_string(&at).unwrap();
        assert_eq!(json, "\"credit_card\"");
        let parsed: AccountType = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, at);
    }

    #[test]
    fn frequency_serde_roundtrip() {
        let f = Frequency::Biweekly;
        let json = serde_json::to_string(&f).unwrap();
        assert_eq!(json, "\"biweekly\"");
        let parsed: Frequency = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, f);
    }

    #[test]
    fn transaction_direction_roundtrip() {
        for variant in [TransactionDirection::Inflow, TransactionDirection::Outflow] {
            let s = variant.to_string();
            let parsed: TransactionDirection = s.parse().unwrap();
            assert_eq!(variant, parsed);
        }
    }

    #[test]
    fn transaction_direction_serializes_snake_case() {
        assert_eq!(
            serde_json::to_string(&TransactionDirection::Inflow).unwrap(),
            "\"inflow\""
        );
        assert_eq!(
            serde_json::to_string(&TransactionDirection::Outflow).unwrap(),
            "\"outflow\""
        );
    }

    #[test]
    fn transaction_direction_invalid_str() {
        assert!("sideways".parse::<TransactionDirection>().is_err());
    }

    #[test]
    fn account_role_for_assets() {
        for ty in [
            AccountType::Checking,
            AccountType::Savings,
            AccountType::Cash,
            AccountType::Crypto,
            AccountType::InvestmentBrokerage,
            AccountType::InvestmentRetirement,
        ] {
            assert_eq!(AccountRole::for_account_type(ty), AccountRole::Asset);
        }
    }

    #[test]
    fn account_role_for_liabilities() {
        for ty in [
            AccountType::CreditCard,
            AccountType::LoanMortgage,
            AccountType::LoanAuto,
            AccountType::LoanStudent,
            AccountType::LoanPersonal,
        ] {
            assert_eq!(AccountRole::for_account_type(ty), AccountRole::Liability);
        }
    }

    #[test]
    fn classify_balance_asset_account() {
        // Asset account: entire balance flows to the asset column.
        // Negative balances (overdrafts) count as reduced assets, not
        // as liabilities.
        let (a, l) = AccountRole::classify_balance(AccountType::Checking, Decimal::new(1500, 0));
        assert_eq!(a, Decimal::new(1500, 0));
        assert_eq!(l, Decimal::ZERO);

        let (a, l) = AccountRole::classify_balance(AccountType::Savings, Decimal::new(-50, 0));
        assert_eq!(a, Decimal::new(-50, 0));
        assert_eq!(l, Decimal::ZERO);
    }

    #[test]
    fn classify_balance_liability_with_debt() {
        // Credit card with $250 owed (negative canonical balance).
        let (a, l) = AccountRole::classify_balance(AccountType::CreditCard, Decimal::new(-250, 0));
        assert_eq!(a, Decimal::ZERO);
        assert_eq!(l, Decimal::new(250, 0));
    }

    #[test]
    fn classify_balance_liability_with_credit() {
        // Credit card with $80 credit balance (overpaid). Cash-like;
        // should flow to assets, not liabilities.
        let (a, l) = AccountRole::classify_balance(AccountType::CreditCard, Decimal::new(80, 0));
        assert_eq!(a, Decimal::new(80, 0));
        assert_eq!(l, Decimal::ZERO);
    }

    #[test]
    fn classify_balance_zero_is_zero_everywhere() {
        for ty in [
            AccountType::Checking,
            AccountType::CreditCard,
            AccountType::LoanStudent,
        ] {
            let (a, l) = AccountRole::classify_balance(ty, Decimal::ZERO);
            assert_eq!(a, Decimal::ZERO);
            assert_eq!(l, Decimal::ZERO);
        }
    }

    #[test]
    fn is_liability_type_predicate() {
        assert!(AccountRole::is_liability_type(AccountType::CreditCard));
        assert!(AccountRole::is_liability_type(AccountType::LoanMortgage));
        assert!(!AccountRole::is_liability_type(AccountType::Checking));
        assert!(!AccountRole::is_liability_type(AccountType::Other));
    }
}
