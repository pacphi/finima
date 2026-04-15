use regex::RegexSet;
use rust_decimal::Decimal;
use uuid::Uuid;

use crate::types::{CategorizationTier, CategoryAssignment};

/// A single pattern rule mapping a regex to a category.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct PatternRule {
    pub pattern: String,
    pub category: String,
    pub subcategory: String,
    pub confidence: f64,
}

/// Trait for Tier 1 pattern-based categorization.
pub trait PatternMatcher: Send + Sync {
    fn match_pattern(&self, description: &str, amount: Decimal) -> Option<CategoryAssignment>;
}

/// Regex-based pattern engine that evaluates all patterns in a single pass
/// using `RegexSet`.
pub struct PatternEngine {
    regex_set: RegexSet,
    rules: Vec<PatternRule>,
}

impl PatternEngine {
    /// Create a new pattern engine from a list of rules.
    ///
    /// Rules are priority-ordered: the first matching rule wins.
    pub fn new(rules: Vec<PatternRule>) -> Self {
        let patterns: Vec<&str> = rules.iter().map(|r| r.pattern.as_str()).collect();
        let regex_set = RegexSet::new(&patterns).expect("all pattern rules must be valid regex");
        Self { regex_set, rules }
    }

    /// Create a pattern engine with the default built-in rules.
    pub fn with_defaults() -> Self {
        Self::new(default_rules())
    }

    /// Match a transaction description (and optionally its amount) against all patterns.
    pub fn match_pattern(&self, description: &str, amount: Decimal) -> Option<CategoryAssignment> {
        let lower = description.to_lowercase();

        // Try regex patterns first (priority-ordered, first match wins)
        let matches: Vec<usize> = self.regex_set.matches(&lower).into_iter().collect();
        if let Some(&first_match) = matches.first() {
            let rule = &self.rules[first_match];
            return Some(CategoryAssignment {
                transaction_id: Uuid::nil(),
                category: rule.category.clone(),
                subcategory: rule.subcategory.clone(),
                merchant_name: String::new(),
                confidence: rule.confidence,
                source_tier: CategorizationTier::PatternEngine,
            });
        }

        // Amount-based heuristics
        if let Some(assignment) = self.amount_heuristics(&lower, amount) {
            return Some(assignment);
        }

        None
    }

    /// Amount-range heuristics for when no regex pattern matched.
    fn amount_heuristics(&self, description: &str, amount: Decimal) -> Option<CategoryAssignment> {
        let zero = Decimal::ZERO;
        let threshold_500 = Decimal::new(500, 0);

        // Positive amounts > $500 with payroll/salary keywords -> income
        if amount > threshold_500 {
            let income_keywords = [
                "payroll",
                "salary",
                "direct dep",
                "direct deposit",
                "ach deposit",
            ];
            for kw in &income_keywords {
                if description.contains(kw) {
                    return Some(CategoryAssignment {
                        transaction_id: Uuid::nil(),
                        category: "income".to_string(),
                        subcategory: "salary".to_string(),
                        merchant_name: String::new(),
                        confidence: 0.85,
                        source_tier: CategorizationTier::PatternEngine,
                    });
                }
            }
        }

        // "PAYMENT" or "THANK YOU" -> credit card payment
        if description.contains("payment thank you")
            || description.contains("payment received")
            || (description.contains("payment") && description.contains("thank you"))
        {
            return Some(CategoryAssignment {
                transaction_id: Uuid::nil(),
                category: "debt_payment".to_string(),
                subcategory: "credit_card_payment".to_string(),
                merchant_name: String::new(),
                confidence: 0.80,
                source_tier: CategorizationTier::PatternEngine,
            });
        }

        // Large positive amounts with no other signal -> possibly income
        if amount > threshold_500 && amount > zero {
            // Check for generic deposit terms
            let deposit_keywords = ["deposit", "xfer", "transfer from"];
            for kw in &deposit_keywords {
                if description.contains(kw) {
                    return Some(CategoryAssignment {
                        transaction_id: Uuid::nil(),
                        category: "transfer".to_string(),
                        subcategory: "internal_transfer".to_string(),
                        merchant_name: String::new(),
                        confidence: 0.60,
                        source_tier: CategorizationTier::PatternEngine,
                    });
                }
            }
        }

        None
    }

    /// Number of rules in the engine.
    pub fn rule_count(&self) -> usize {
        self.rules.len()
    }
}

impl PatternMatcher for PatternEngine {
    fn match_pattern(&self, description: &str, amount: Decimal) -> Option<CategoryAssignment> {
        self.match_pattern(description, amount)
    }
}

/// Default set of ~35 pattern rules covering common transaction types.
pub fn default_rules() -> Vec<PatternRule> {
    vec![
        // ── Income ──
        PatternRule {
            pattern: r"(?i)payroll|salary|direct.dep(osit)?|ach.deposit".to_string(),
            category: "income".to_string(),
            subcategory: "salary".to_string(),
            confidence: 0.90,
        },
        PatternRule {
            pattern: r"(?i)interest.paid|interest.payment|interest.earned".to_string(),
            category: "income".to_string(),
            subcategory: "interest".to_string(),
            confidence: 0.85,
        },
        PatternRule {
            pattern: r"(?i)dividend|div.payment".to_string(),
            category: "income".to_string(),
            subcategory: "dividends".to_string(),
            confidence: 0.85,
        },
        PatternRule {
            pattern: r"(?i)refund|rebate|cashback|cash\s*back".to_string(),
            category: "income".to_string(),
            subcategory: "refunds".to_string(),
            confidence: 0.80,
        },

        // ── Streaming / Entertainment ──
        PatternRule {
            pattern: r"(?i)netflix".to_string(),
            category: "entertainment".to_string(),
            subcategory: "streaming_services".to_string(),
            confidence: 0.95,
        },
        PatternRule {
            pattern: r"(?i)hulu".to_string(),
            category: "entertainment".to_string(),
            subcategory: "streaming_services".to_string(),
            confidence: 0.95,
        },
        PatternRule {
            pattern: r"(?i)disney\s*\+|disneyplus".to_string(),
            category: "entertainment".to_string(),
            subcategory: "streaming_services".to_string(),
            confidence: 0.95,
        },
        PatternRule {
            pattern: r"(?i)spotify|apple\s*music|pandora|tidal|deezer".to_string(),
            category: "entertainment".to_string(),
            subcategory: "streaming_services".to_string(),
            confidence: 0.95,
        },
        PatternRule {
            pattern: r"(?i)hbo\s*max|max\.com|peacock|paramount\+|youtube\s*(premium|tv)".to_string(),
            category: "entertainment".to_string(),
            subcategory: "streaming_services".to_string(),
            confidence: 0.95,
        },
        PatternRule {
            pattern: r"(?i)audible|kindle\s*unlimited".to_string(),
            category: "entertainment".to_string(),
            subcategory: "books_media".to_string(),
            confidence: 0.90,
        },

        // ── Food Delivery (must come before rideshare so Uber Eats matches first) ──
        PatternRule {
            pattern: r"(?i)uber\s*eat".to_string(),
            category: "food_dining".to_string(),
            subcategory: "food_delivery".to_string(),
            confidence: 0.90,
        },
        PatternRule {
            pattern: r"(?i)doordash|grubhub|postmates|seamless|instacart".to_string(),
            category: "food_dining".to_string(),
            subcategory: "food_delivery".to_string(),
            confidence: 0.90,
        },

        // ── Transportation ──
        PatternRule {
            pattern: r"(?i)\buber\b|lyft".to_string(),
            category: "transportation".to_string(),
            subcategory: "rideshare_taxi".to_string(),
            confidence: 0.90,
        },

        // ── Shopping ──
        PatternRule {
            pattern: r"(?i)amazon|amzn".to_string(),
            category: "shopping".to_string(),
            subcategory: "online_shopping".to_string(),
            confidence: 0.65, // Amazon is ambiguous
        },
        PatternRule {
            pattern: r"(?i)walmart|wal-mart|wal\s+mart".to_string(),
            category: "shopping".to_string(),
            subcategory: "general_merchandise".to_string(),
            confidence: 0.80,
        },
        PatternRule {
            pattern: r"(?i)target\s".to_string(),
            category: "shopping".to_string(),
            subcategory: "general_merchandise".to_string(),
            confidence: 0.80,
        },
        PatternRule {
            pattern: r"(?i)costco|sams?\s*club|bj'?s\s*wholesale".to_string(),
            category: "shopping".to_string(),
            subcategory: "general_merchandise".to_string(),
            confidence: 0.85,
        },
        PatternRule {
            pattern: r"(?i)ebay|etsy|shopify".to_string(),
            category: "shopping".to_string(),
            subcategory: "online_shopping".to_string(),
            confidence: 0.75,
        },

        // ── Bills & Utilities ──
        PatternRule {
            pattern: r"(?i)comcast|xfinity|spectrum|at&?t|t-mobile|verizon|sprint|mint\s*mobile".to_string(),
            category: "bills_utilities".to_string(),
            subcategory: "phone".to_string(),
            confidence: 0.85,
        },
        PatternRule {
            pattern: r"(?i)electric|power\s*co|utility|water\s*(dept|district|co)|sewer|gas\s*co".to_string(),
            category: "bills_utilities".to_string(),
            subcategory: "utilities".to_string(),
            confidence: 0.80,
        },

        // ── Housing ──
        PatternRule {
            pattern: r"(?i)rent\s*(payment)?|landlord|property\s*mgmt|apartment".to_string(),
            category: "housing".to_string(),
            subcategory: "rent".to_string(),
            confidence: 0.80,
        },
        PatternRule {
            pattern: r"(?i)mortgage|home\s*loan|escrow".to_string(),
            category: "housing".to_string(),
            subcategory: "mortgage".to_string(),
            confidence: 0.85,
        },

        // ── Insurance ──
        PatternRule {
            pattern: r"(?i)geico|allstate|state\s*farm|progressive|liberty\s*mutual|farmers\s*ins|usaa".to_string(),
            category: "insurance".to_string(),
            subcategory: "auto_insurance".to_string(),
            confidence: 0.85,
        },

        // ── Health ──
        PatternRule {
            pattern: r"(?i)cvs|walgreens|rite\s*aid|pharmacy".to_string(),
            category: "health_wellness".to_string(),
            subcategory: "pharmacy".to_string(),
            confidence: 0.80,
        },
        PatternRule {
            pattern: r"(?i)gym|fitness|planet\s*fit|la\s*fitness|orangetheory|crossfit|ymca|ywca".to_string(),
            category: "recreation".to_string(),
            subcategory: "gym_fitness".to_string(),
            confidence: 0.85,
        },

        // ── Subscriptions / Software ──
        PatternRule {
            pattern: r"(?i)github|gitlab|aws|azure|google\s*cloud|digitalocean|heroku".to_string(),
            category: "bills_utilities".to_string(),
            subcategory: "software_subscriptions".to_string(),
            confidence: 0.80,
        },
        PatternRule {
            pattern: r"(?i)adobe|microsoft\s*365|office\s*365|dropbox|icloud|google\s*(one|storage)".to_string(),
            category: "bills_utilities".to_string(),
            subcategory: "software_subscriptions".to_string(),
            confidence: 0.85,
        },

        // ── Credit Card Payments ──
        PatternRule {
            pattern: r"(?i)chase\s*credit|chase\s*crd|chase\s*card".to_string(),
            category: "debt_payment".to_string(),
            subcategory: "credit_card_payment".to_string(),
            confidence: 0.90,
        },
        PatternRule {
            pattern: r"(?i)bk\s*of\s*amer|bank\s*of\s*america|bofa".to_string(),
            category: "debt_payment".to_string(),
            subcategory: "credit_card_payment".to_string(),
            confidence: 0.85,
        },
        PatternRule {
            pattern: r"(?i)amex|american\s*express|epayment".to_string(),
            category: "debt_payment".to_string(),
            subcategory: "credit_card_payment".to_string(),
            confidence: 0.85,
        },
        PatternRule {
            pattern: r"(?i)discover\s*(dc|card|pay|fin)".to_string(),
            category: "debt_payment".to_string(),
            subcategory: "credit_card_payment".to_string(),
            confidence: 0.85,
        },
        PatternRule {
            pattern: r"(?i)applecard|apple\s*card|gsbank".to_string(),
            category: "debt_payment".to_string(),
            subcategory: "credit_card_payment".to_string(),
            confidence: 0.85,
        },
        PatternRule {
            pattern: r"(?i)barclaycard|barclay".to_string(),
            category: "debt_payment".to_string(),
            subcategory: "credit_card_payment".to_string(),
            confidence: 0.85,
        },
        PatternRule {
            pattern: r"(?i)capital\s*one|citi\s*card|citibank.*pay".to_string(),
            category: "debt_payment".to_string(),
            subcategory: "credit_card_payment".to_string(),
            confidence: 0.85,
        },
        PatternRule {
            pattern: r"(?i)(e-?pay(ment)?|online\s*pmt|creditcard|credit\s*card)\s*$".to_string(),
            category: "debt_payment".to_string(),
            subcategory: "credit_card_payment".to_string(),
            confidence: 0.75,
        },

        // ── Loan Payments ──
        PatternRule {
            pattern: r"(?i)loan\s*pay(m(en)?t)?|student\s*loan|auto\s*loan".to_string(),
            category: "debt_payment".to_string(),
            subcategory: "loan_payment".to_string(),
            confidence: 0.85,
        },

        // ── Bank Transfers (internal) ──
        PatternRule {
            pattern: r"(?i)online\s*banking\s*transfer|transfer\s*(to|from)\s*\d".to_string(),
            category: "transfer".to_string(),
            subcategory: "savings_transfer".to_string(),
            confidence: 0.85,
        },
        PatternRule {
            pattern: r"(?i)webxfr|web\s*transfer|onlne\s*trnsfr".to_string(),
            category: "transfer".to_string(),
            subcategory: "checking_transfer".to_string(),
            confidence: 0.80,
        },
        PatternRule {
            pattern: r"(?i)overdraft\s*protection\s*deposit".to_string(),
            category: "transfer".to_string(),
            subcategory: "checking_transfer".to_string(),
            confidence: 0.90,
        },

        // ── Peer-to-Peer ──
        PatternRule {
            pattern: r"(?i)venmo|zelle|cashapp|cash\s*app|paypal".to_string(),
            category: "transfer".to_string(),
            subcategory: "peer_to_peer".to_string(),
            confidence: 0.70,
        },
        PatternRule {
            pattern: r"(?i)wire\s*transfer|ach\s*(credit|debit|transfer)".to_string(),
            category: "transfer".to_string(),
            subcategory: "ach_transfer".to_string(),
            confidence: 0.75,
        },

        // ── P2P online transfers (e.g. "ONLINE TRANSFER...P2P") ──
        PatternRule {
            pattern: r"(?i)online\s*transfer.*p2p|p2p.*online\s*transfer|p2p\s*(transfer|payment|pay)".to_string(),
            category: "transfer".to_string(),
            subcategory: "peer_to_peer".to_string(),
            confidence: 0.80,
        },

        // ── External bank withdrawals / loan payments ──
        // Descriptions like "External Withdrawal - Cornerstone Bank - Payment"
        PatternRule {
            pattern: r"(?i)external\s*(withdrawal|debit).*bank".to_string(),
            category: "debt_payment".to_string(),
            subcategory: "loan_payment".to_string(),
            confidence: 0.75,
        },

        // ── Mobile banking deposits / internal deposits ──
        // Descriptions like "Deposit - Mobile Banking"
        PatternRule {
            pattern: r"(?i)(deposit|credit)\s*[-–]?\s*mobile\s*bank(ing)?".to_string(),
            category: "transfer".to_string(),
            subcategory: "internal_transfer".to_string(),
            confidence: 0.80,
        },

        // ── External deposits from named individuals / P2P ──
        // Descriptions like "External Deposit - CHRIS PHILLIPSON ONLINE TRANSFER..."
        PatternRule {
            pattern: r"(?i)external\s*deposit.*online\s*transfer|external\s*deposit.*transfer".to_string(),
            category: "transfer".to_string(),
            subcategory: "peer_to_peer".to_string(),
            confidence: 0.75,
        },

        // ── Utilities (specific companies) ──
        PatternRule {
            pattern: r"(?i)puget\s*sound\s*ener|pse\.com|seattle\s*city\s*light".to_string(),
            category: "utilities".to_string(),
            subcategory: "electricity".to_string(),
            confidence: 0.90,
        },
        PatternRule {
            pattern: r"(?i)waste\s*management|republic\s*services|recology".to_string(),
            category: "utilities".to_string(),
            subcategory: "trash_recycling".to_string(),
            confidence: 0.90,
        },

        // ── Government / Taxes ──
        PatternRule {
            pattern: r"(?i)county\s*(tax|treas)|property\s*tax|snohomish|king\s*county".to_string(),
            category: "housing".to_string(),
            subcategory: "property_tax".to_string(),
            confidence: 0.85,
        },
        PatternRule {
            pattern: r"(?i)mill\s*creek\s*count|hoa|homeowner".to_string(),
            category: "housing".to_string(),
            subcategory: "hoa_fees".to_string(),
            confidence: 0.80,
        },

        // ── Healthcare ──
        PatternRule {
            pattern: r"(?i)optum|united\s*health|kaiser|premera|regence|cigna|aetna|anthem|humana".to_string(),
            category: "healthcare".to_string(),
            subcategory: "health_insurance".to_string(),
            confidence: 0.85,
        },
        PatternRule {
            pattern: r"(?i)providence|virginia\s*mason|swedish|multicare|overlake".to_string(),
            category: "healthcare".to_string(),
            subcategory: "doctor_visits".to_string(),
            confidence: 0.80,
        },

        // ── Pest Control / Home Services ──
        PatternRule {
            pattern: r"(?i)aptive|terminix|orkin|pest".to_string(),
            category: "housing".to_string(),
            subcategory: "home_maintenance".to_string(),
            confidence: 0.85,
        },

        // ── ATM ──
        PatternRule {
            pattern: r"(?i)atm\s*(withdrawal|w/d|cash|inquiry)|cash\s*withdrawal".to_string(),
            category: "other".to_string(),
            subcategory: "cash_withdrawal".to_string(),
            confidence: 0.90,
        },

        // ── Check ──
        PatternRule {
            pattern: r"(?i)^check$|check\s*#?\d".to_string(),
            category: "other".to_string(),
            subcategory: "miscellaneous".to_string(),
            confidence: 0.60,
        },

        // ── NSF / Overdraft ──
        PatternRule {
            pattern: r"(?i)^nsf\b|nsf\s*fee|nsf\s*-".to_string(),
            category: "fees_charges".to_string(),
            subcategory: "overdraft_fees".to_string(),
            confidence: 0.90,
        },

        // ── Interest Charges ──
        PatternRule {
            pattern: r"(?i)interest\s*charged|interest\s*on\s*purchases|finance\s*charge".to_string(),
            category: "fees_charges".to_string(),
            subcategory: "late_fees".to_string(),
            confidence: 0.85,
        },

        // ── Government Benefits ──
        PatternRule {
            pattern: r"(?i)ui\s*benefit|unemploy|employ\s*sec|esd\s*benefit".to_string(),
            category: "income".to_string(),
            subcategory: "government_benefits".to_string(),
            confidence: 0.90,
        },

        // ── BA Electronic Payment (generic credit card) ──
        PatternRule {
            pattern: r"(?i)ba\s*electronic\s*payment".to_string(),
            category: "debt_payment".to_string(),
            subcategory: "credit_card_payment".to_string(),
            confidence: 0.80,
        },

        // ── Deposit from external ──
        PatternRule {
            pattern: r"(?i)deposit.*online\s*banking\s*transfer\s*from".to_string(),
            category: "transfer".to_string(),
            subcategory: "savings_transfer".to_string(),
            confidence: 0.85,
        },

        // ── Fees ──
        PatternRule {
            pattern: r"(?i)overdraft\s*fee|nsf\s*fee|monthly\s*(service\s*)?fee|maintenance\s*fee".to_string(),
            category: "fees".to_string(),
            subcategory: "bank_fees".to_string(),
            confidence: 0.90,
        },
        PatternRule {
            pattern: r"(?i)late\s*fee|penalty|finance\s*charge".to_string(),
            category: "fees".to_string(),
            subcategory: "late_fees".to_string(),
            confidence: 0.85,
        },

        // ── Education ──
        PatternRule {
            pattern: r"(?i)tuition|university|college|student\s*loan|navient|sallie\s*mae|nelnet".to_string(),
            category: "education".to_string(),
            subcategory: "tuition".to_string(),
            confidence: 0.85,
        },

        // ── Childcare ──
        PatternRule {
            pattern: r"(?i)daycare|childcare|preschool|montessori|kindercare".to_string(),
            category: "personal_care".to_string(),
            subcategory: "childcare".to_string(),
            confidence: 0.85,
        },

        // ── Travel ──
        PatternRule {
            pattern: r"(?i)marriott|hilton|hyatt|airbnb|vrbo|hotel|motel|inn\s".to_string(),
            category: "travel".to_string(),
            subcategory: "lodging".to_string(),
            confidence: 0.85,
        },
        PatternRule {
            pattern: r"(?i)united\s*air|delta\s*air|american\s*air|southwest\s*air|jetblue|frontier\s*air|spirit\s*air|alaska\s*air".to_string(),
            category: "travel".to_string(),
            subcategory: "flights".to_string(),
            confidence: 0.90,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pattern_matches_netflix() {
        let engine = PatternEngine::with_defaults();
        let result = engine.match_pattern("NETFLIX.COM", Decimal::new(-1599, 2));
        assert!(result.is_some());
        let a = result.unwrap();
        assert_eq!(a.category, "entertainment");
        assert_eq!(a.subcategory, "streaming_services");
    }

    #[test]
    fn pattern_matches_uber_rideshare() {
        let engine = PatternEngine::with_defaults();
        let result = engine.match_pattern("UBER TRIP ABC123", Decimal::new(-2450, 2));
        assert!(result.is_some());
        let a = result.unwrap();
        assert_eq!(a.category, "transportation");
        assert_eq!(a.subcategory, "rideshare_taxi");
    }

    #[test]
    fn pattern_matches_uber_eats() {
        let engine = PatternEngine::with_defaults();
        let result = engine.match_pattern("UBER EATS ORDER", Decimal::new(-3200, 2));
        assert!(result.is_some());
        let a = result.unwrap();
        assert_eq!(a.category, "food_dining");
        assert_eq!(a.subcategory, "food_delivery");
    }

    #[test]
    fn pattern_matches_payroll() {
        let engine = PatternEngine::with_defaults();
        let result = engine.match_pattern("PAYROLL DEPOSIT ACME CORP", Decimal::new(350000, 2));
        assert!(result.is_some());
        let a = result.unwrap();
        assert_eq!(a.category, "income");
        assert_eq!(a.subcategory, "salary");
    }

    #[test]
    fn pattern_matches_amazon_low_confidence() {
        let engine = PatternEngine::with_defaults();
        let result = engine.match_pattern("AMAZON.COM*A1B2C3", Decimal::new(-4599, 2));
        assert!(result.is_some());
        let a = result.unwrap();
        assert_eq!(a.category, "shopping");
        assert!(a.confidence < 0.70);
    }

    #[test]
    fn amount_heuristic_payment_thank_you() {
        let engine = PatternEngine::with_defaults();
        let result = engine.match_pattern("PAYMENT THANK YOU", Decimal::new(-50000, 2));
        assert!(result.is_some());
        let a = result.unwrap();
        assert_eq!(a.category, "debt_payment");
        assert_eq!(a.subcategory, "credit_card_payment");
    }

    #[test]
    fn no_match_returns_none() {
        let engine = PatternEngine::with_defaults();
        let result = engine.match_pattern("OBSCURE MERCHANT XYZ", Decimal::new(-1234, 2));
        assert!(result.is_none());
    }

    #[test]
    fn default_rules_count() {
        let engine = PatternEngine::with_defaults();
        assert!(engine.rule_count() >= 30);
    }

    #[test]
    fn pattern_matches_external_bank_withdrawal() {
        let engine = PatternEngine::with_defaults();
        let result = engine.match_pattern(
            "External Withdrawal - Cornerstone Bank - Payment",
            Decimal::new(-125000, 2),
        );
        assert!(result.is_some());
        let a = result.unwrap();
        assert_eq!(a.category, "debt_payment");
        assert_eq!(a.subcategory, "loan_payment");
    }

    #[test]
    fn pattern_matches_mobile_banking_deposit() {
        let engine = PatternEngine::with_defaults();
        let result = engine.match_pattern("Deposit - Mobile Banking", Decimal::new(50000, 2));
        assert!(result.is_some());
        let a = result.unwrap();
        assert_eq!(a.category, "transfer");
        assert_eq!(a.subcategory, "internal_transfer");
    }

    #[test]
    fn pattern_matches_external_deposit_online_transfer() {
        let engine = PatternEngine::with_defaults();
        let result = engine.match_pattern(
            "External Deposit - CHRIS PHILLIPSON ONLINE TRANSFERXXXXXXXXXX - P2P",
            Decimal::new(75000, 2),
        );
        assert!(result.is_some());
        let a = result.unwrap();
        assert_eq!(a.category, "transfer");
    }

    #[test]
    fn pattern_matches_p2p_transfer() {
        let engine = PatternEngine::with_defaults();
        let result = engine.match_pattern("ONLINE TRANSFER 123456 P2P", Decimal::new(20000, 2));
        assert!(result.is_some());
        let a = result.unwrap();
        assert_eq!(a.category, "transfer");
        assert_eq!(a.subcategory, "peer_to_peer");
    }
}
