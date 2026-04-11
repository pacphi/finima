/// Normalize a raw transaction description into a clean merchant name.
///
/// Applies rule-based cleanup:
/// - Strips common prefixes: "SQ *", "AMZN*", "CKE*", "TST*"
/// - Strips card numbers and location codes (e.g., "#10432")
/// - Maps known abbreviations to full names
/// - Applies title case to the result
pub fn normalize_merchant(description: &str) -> String {
    let mut name = description.trim().to_string();

    // Strip common payment processor prefixes
    let prefixes = ["SQ *", "SQ*", "TST*", "TST *", "CKE*", "CKE *"];
    for prefix in &prefixes {
        if let Some(stripped) = name.strip_prefix(prefix) {
            name = stripped.trim().to_string();
            break;
        }
    }

    // Handle Amazon abbreviations — must check before stripping suffixes
    if name.starts_with("AMZN*") || name.starts_with("AMZN MKTP") || name.starts_with("AMZN ") {
        return "Amazon".to_string();
    }

    // Handle known abbreviations
    let lower = name.to_lowercase();
    if lower.starts_with("wholefds") || lower.starts_with("whole fds") {
        return "Whole Foods Market".to_string();
    }
    if lower.starts_with("wm supercenter") || lower == "walmart" {
        return "Walmart".to_string();
    }
    if lower.starts_with("costco whse") || lower.starts_with("costco") {
        return "Costco".to_string();
    }
    if lower.starts_with("tgt ") || lower.starts_with("target") {
        return "Target".to_string();
    }

    // Strip trailing location/store codes like "#10432", "# 10432"
    if let Some(hash_pos) = name.find('#') {
        name = name[..hash_pos].trim().to_string();
    }

    // Strip trailing numeric sequences (card numbers, reference IDs)
    name = strip_trailing_numbers(&name);

    // Title case
    titlecase(&name)
}

/// Strip trailing numeric sequences and whitespace.
fn strip_trailing_numbers(s: &str) -> String {
    let trimmed = s.trim_end();
    let result: &str =
        trimmed.trim_end_matches(|c: char| c.is_ascii_digit() || c == ' ' || c == '-');
    if result.is_empty() {
        return trimmed.to_string();
    }
    result.trim().to_string()
}

/// Convert a string to title case.
fn titlecase(s: &str) -> String {
    s.split_whitespace()
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(first) => {
                    let upper: String = first.to_uppercase().collect();
                    let lower: String = chars.collect::<String>().to_lowercase();
                    format!("{}{}", upper, lower)
                }
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}
