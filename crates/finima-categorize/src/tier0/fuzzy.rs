/// Compute the Jaro-Winkler similarity between two strings.
///
/// Returns a value in `[0.0, 1.0]` where 1.0 is an exact match.
pub fn jaro_winkler(a: &str, b: &str) -> f64 {
    strsim::jaro_winkler(a, b)
}

/// Find the best fuzzy match among `candidates` for the given `query`.
///
/// Returns `(index, similarity)` of the best candidate that meets
/// the `threshold`, or `None` if no candidate qualifies.
pub fn best_match(query: &str, candidates: &[String], threshold: f64) -> Option<(usize, f64)> {
    let mut best_idx = 0;
    let mut best_score = 0.0_f64;

    for (i, candidate) in candidates.iter().enumerate() {
        let score = jaro_winkler(query, candidate);
        if score > best_score {
            best_score = score;
            best_idx = i;
        }
    }

    if best_score >= threshold {
        Some((best_idx, best_score))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_match_returns_one() {
        assert!((jaro_winkler("starbucks", "starbucks") - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn similar_strings_score_high() {
        let score = jaro_winkler("starbucks", "starbuck");
        assert!(score > 0.95);
    }

    #[test]
    fn dissimilar_strings_score_low() {
        let score = jaro_winkler("starbucks", "walmart");
        assert!(score < 0.6);
    }

    #[test]
    fn best_match_finds_closest() {
        let candidates = vec![
            "walmart".to_string(),
            "starbuck".to_string(),
            "target".to_string(),
        ];
        let result = best_match("starbucks", &candidates, 0.88);
        assert!(result.is_some());
        let (idx, _score) = result.unwrap();
        assert_eq!(idx, 1);
    }

    #[test]
    fn best_match_returns_none_below_threshold() {
        let candidates = vec!["walmart".to_string(), "target".to_string()];
        let result = best_match("starbucks", &candidates, 0.88);
        assert!(result.is_none());
    }
}
