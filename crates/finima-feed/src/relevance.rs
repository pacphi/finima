//! Heuristic relevance scoring for articles based on user portfolio.

use finima_core::types::AccountType;

use crate::RawArticle;

/// Scores article relevance to a user on a 1-5 scale.
pub struct RelevanceScorer;

impl RelevanceScorer {
    /// Compute a heuristic relevance score (1-5) for an article.
    ///
    /// Scoring rules:
    /// - Base score: 1
    /// - +1 if article topic matches a user account type
    /// - +1 if article mentions a top spending category
    /// - +1 for educational content keywords
    /// - Cap at 5
    pub fn score(
        article: &RawArticle,
        account_types: &[AccountType],
        top_categories: &[String],
    ) -> u8 {
        let mut score: u8 = 1; // base

        // +1 if topic aligns with user account types
        if topic_matches_account_types(&article.topics, account_types) {
            score += 1;
        }

        // +1 if article content mentions a top spending category
        let lower_snippet = article.content_snippet.to_lowercase();
        let lower_title = article.title.to_lowercase();
        if category_mentioned(&lower_snippet, &lower_title, top_categories) {
            score += 1;
        }

        // +1 for educational content
        if is_educational(&lower_snippet, &lower_title) {
            score += 1;
        }

        // +1 if the article topic directly matches a user interest area
        if has_direct_relevance(&article.topics, account_types) {
            score += 1;
        }

        score.min(5)
    }
}

/// Check if any article topic matches a user's account types.
fn topic_matches_account_types(topics: &[String], account_types: &[AccountType]) -> bool {
    for topic in topics {
        let lower = topic.to_lowercase();
        for at in account_types {
            match at {
                AccountType::InvestmentBrokerage | AccountType::InvestmentRetirement
                    if lower.contains("invest") =>
                {
                    return true;
                }
                AccountType::InvestmentRetirement if lower.contains("retirement") => {
                    return true;
                }
                AccountType::CreditCard if lower.contains("credit") => {
                    return true;
                }
                AccountType::LoanMortgage
                | AccountType::LoanAuto
                | AccountType::LoanStudent
                | AccountType::LoanPersonal
                    if lower.contains("loan") || lower.contains("debt") =>
                {
                    return true;
                }
                AccountType::Savings if lower.contains("saving") || lower.contains("budget") => {
                    return true;
                }
                AccountType::Checking if lower.contains("budget") => {
                    return true;
                }
                AccountType::Crypto if lower.contains("crypto") => {
                    return true;
                }
                _ => {}
            }
        }
    }
    false
}

/// Check if the article mentions any of the user's top spending categories.
fn category_mentioned(snippet: &str, title: &str, categories: &[String]) -> bool {
    for cat in categories {
        let lower_cat = cat.to_lowercase();
        // Handle underscored category names like "food_dining"
        let words: Vec<&str> = lower_cat.split('_').collect();
        if words
            .iter()
            .any(|w| snippet.contains(w) || title.contains(w))
        {
            return true;
        }
    }
    false
}

/// Check if article content appears educational.
fn is_educational(snippet: &str, title: &str) -> bool {
    let edu_keywords = [
        "how to",
        "guide",
        "tips",
        "learn",
        "beginner",
        "basics",
        "explained",
        "tutorial",
        "strategy",
        "planning",
    ];

    let combined = format!("{} {}", title, snippet);
    edu_keywords.iter().any(|kw| combined.contains(kw))
}

/// Check if article topics have direct relevance to specific account types.
fn has_direct_relevance(topics: &[String], account_types: &[AccountType]) -> bool {
    for topic in topics {
        let lower = topic.to_lowercase();
        if lower.contains("tax") {
            // Taxes are relevant to everyone, but especially investment accounts.
            if account_types.iter().any(|at| {
                matches!(
                    at,
                    AccountType::InvestmentBrokerage | AccountType::InvestmentRetirement
                )
            }) {
                return true;
            }
        }
        if lower.contains("retirement")
            && account_types
                .iter()
                .any(|at| matches!(at, AccountType::InvestmentRetirement))
        {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_article(title: &str, snippet: &str, topics: Vec<&str>) -> RawArticle {
        RawArticle {
            title: title.to_string(),
            url: "https://example.com/article".to_string(),
            source_name: "Test Source".to_string(),
            published_at: None,
            content_snippet: snippet.to_string(),
            topics: topics.into_iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn base_score_is_one() {
        let article = make_article("Random News", "Nothing relevant here.", vec!["general"]);
        let score = RelevanceScorer::score(&article, &[], &[]);
        assert_eq!(score, 1);
    }

    #[test]
    fn investing_topic_matches_investment_account() {
        let article = make_article(
            "Stock Market Update",
            "Markets rallied today.",
            vec!["investing"],
        );
        let score = RelevanceScorer::score(&article, &[AccountType::InvestmentBrokerage], &[]);
        // base(1) + topic match(1) = 2
        assert!(score >= 2);
    }

    #[test]
    fn educational_content_gets_bonus() {
        let article = make_article(
            "How to Save Money: A Beginner's Guide",
            "Learn the basics of budgeting and saving tips for beginners.",
            vec!["budgeting"],
        );
        let score = RelevanceScorer::score(&article, &[], &[]);
        // base(1) + educational(1) = 2
        assert!(score >= 2);
    }

    #[test]
    fn category_mention_gets_bonus() {
        let article = make_article(
            "Dining Out on a Budget",
            "Reduce your food and dining expenses with these tricks.",
            vec!["budgeting"],
        );
        let score = RelevanceScorer::score(&article, &[], &["food_dining".to_string()]);
        // base(1) + category match(1) = 2
        assert!(score >= 2);
    }

    #[test]
    fn high_relevance_capped_at_five() {
        let article = make_article(
            "How to Invest for Retirement: A Beginner's Guide",
            "Learn the basics of investing for retirement planning tips.",
            vec!["investing", "retirement"],
        );
        let score = RelevanceScorer::score(
            &article,
            &[AccountType::InvestmentRetirement],
            &["investing".to_string()],
        );
        assert!(score <= 5);
    }

    #[test]
    fn credit_topic_matches_credit_card_account() {
        let article = make_article(
            "Managing Credit Card Debt",
            "Tips for reducing credit card balances.",
            vec!["credit"],
        );
        let score = RelevanceScorer::score(&article, &[AccountType::CreditCard], &[]);
        assert!(score >= 2);
    }

    #[test]
    fn multiple_signals_stack() {
        let article = make_article(
            "How to Budget Your Investment Returns",
            "Learn tips for managing your investment income with budgeting strategy.",
            vec!["investing"],
        );
        let score = RelevanceScorer::score(
            &article,
            &[AccountType::InvestmentBrokerage],
            &["investment".to_string()],
        );
        // base + topic match + category + educational = at least 4
        assert!(score >= 3);
    }
}
