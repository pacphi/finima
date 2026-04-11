use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// All supported account types in Finima.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
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
    Complete,
    Error,
}

impl fmt::Display for UploadStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::Pending => "pending",
            Self::Processing => "processing",
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
}
