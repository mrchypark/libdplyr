//! Input validation and safeguards (DoS/malicious patterns).

use crate::error::TranspileError;
use crate::options::{MAX_FUNCTION_CALLS, MAX_NESTING_DEPTH};

// R9-AC2: Security validation functions for malicious input detection
pub(crate) fn validate_input_security(input: &str) -> Result<(), TranspileError> {
    // Check for excessive nesting depth
    let nesting_depth = calculate_nesting_depth(input);
    if nesting_depth > MAX_NESTING_DEPTH {
        return Err(TranspileError::internal_error_with_hint(
            &format!(
                "Excessive nesting depth: {} exceeds maximum {}",
                nesting_depth, MAX_NESTING_DEPTH
            ),
            Some("Reduce nested function calls or parentheses".to_string()),
        ));
    }

    // Check for excessive function calls
    let function_count = count_function_calls(input);
    if function_count > MAX_FUNCTION_CALLS {
        return Err(TranspileError::internal_error_with_hint(
            &format!(
                "Too many function calls: {} exceeds maximum {}",
                function_count, MAX_FUNCTION_CALLS
            ),
            Some("Simplify the dplyr pipeline".to_string()),
        ));
    }

    // Check for suspicious patterns that might indicate malicious input
    if contains_suspicious_patterns(input) {
        return Err(TranspileError::internal_error_with_hint(
            "Input contains potentially malicious patterns",
            Some("Remove suspicious characters or patterns".to_string()),
        ));
    }

    // Check for excessive repetition (potential DoS pattern)
    if has_excessive_repetition(input) {
        return Err(TranspileError::internal_error_with_hint(
            "Input contains excessive repetition patterns",
            Some("Reduce repetitive patterns in input".to_string()),
        ));
    }

    Ok(())
}

pub(crate) fn calculate_nesting_depth(input: &str) -> usize {
    let mut max_depth = 0;
    let mut current_depth: i32 = 0;

    for ch in input.chars() {
        match ch {
            '(' | '[' | '{' => {
                current_depth += 1;
                max_depth = max_depth.max(current_depth);
            }
            ')' | ']' | '}' => {
                if current_depth > 0 {
                    current_depth -= 1;
                }
            }
            _ => {}
        }
    }

    max_depth.try_into().unwrap()
}

pub(crate) fn count_function_calls(input: &str) -> usize {
    // Count patterns that look like function calls: identifier followed by '('
    let mut count = 0;
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        if chars[i].is_alphabetic() || chars[i] == '_' {
            // Found start of identifier
            while i < chars.len()
                && (chars[i].is_alphanumeric() || chars[i] == '_' || chars[i] == '.')
            {
                i += 1;
            }

            // Skip whitespace
            while i < chars.len() && chars[i].is_whitespace() {
                i += 1;
            }

            // Check if followed by '('
            if i < chars.len() && chars[i] == '(' {
                count += 1;
            }
        } else {
            i += 1;
        }
    }

    count
}

pub(crate) fn contains_suspicious_patterns(input: &str) -> bool {
    // Check for patterns that might indicate injection attempts or malicious input
    let suspicious_patterns = [
        // SQL injection patterns
        "'; DROP",
        "'; DELETE",
        "'; INSERT",
        "'; UPDATE",
        "UNION SELECT",
        "OR 1=1",
        "AND 1=1",
        // Script injection patterns
        "<script",
        "javascript:",
        "eval(",
        "exec(",
        // Path traversal patterns
        "../",
        "..\\",
        // Null bytes and control characters
        "\0",
        "\x01",
        "\x02",
        "\x03",
        "\x04",
        "\x05",
        "\x06",
        "\x07",
        "\x08",
        "\x0B",
        "\x0C",
        "\x0E",
        "\x0F",
        // Excessive special characters
    ];

    let input_upper = input.to_uppercase();
    for pattern in &suspicious_patterns {
        if input_upper.contains(&pattern.to_uppercase()) {
            return true;
        }
    }

    // Check for excessive special characters (potential obfuscation)
    let special_char_count = input
        .chars()
        .filter(|&c| {
            !c.is_alphanumeric() && !c.is_whitespace() && !"()[]{},.;:_-+*/%><=!&|".contains(c)
        })
        .count();

    if special_char_count > input.len() / 10 {
        return true; // More than 10% special characters
    }

    false
}

pub(crate) fn has_excessive_repetition(input: &str) -> bool {
    // Check for patterns that repeat excessively (potential DoS)
    let chars: Vec<char> = input.chars().collect();

    // Check for repeated characters
    let mut consecutive_count = 1;
    for i in 1..chars.len() {
        if chars[i] == chars[i - 1] {
            consecutive_count += 1;
            if consecutive_count > 100 {
                return true; // More than 100 consecutive identical characters
            }
        } else {
            consecutive_count = 1;
        }
    }

    // Check for repeated substrings
    for pattern_len in 2..=10 {
        if pattern_len * 20 > input.len() {
            break;
        }

        let mut pattern_counts = std::collections::HashMap::new();
        for i in 0..=(chars.len() - pattern_len) {
            let pattern: String = chars[i..i + pattern_len].iter().collect();
            *pattern_counts.entry(pattern).or_insert(0) += 1;
        }

        // If any pattern repeats more than 20 times, consider it excessive
        if pattern_counts.values().any(|&count| count > 20) {
            return true;
        }
    }

    false
}

// R9-AC2: Additional input validation functions
pub(crate) fn validate_input_encoding(input: &str) -> Result<(), TranspileError> {
    // Check for valid UTF-8 (already done by CStr::to_str, but double-check)
    // Check all characters for control characters and confusing Unicode
    for ch in input.chars() {
        // Check for control characters (except common whitespace)
        if ch.is_control() && !matches!(ch, '\t' | '\n' | '\r') {
            return Err(TranspileError::invalid_utf8_error(&format!(
                "Contains control character: U+{:04X}",
                ch as u32
            )));
        }

        // Check for potentially confusing Unicode characters
        if is_confusing_unicode(ch) {
            return Err(TranspileError::invalid_utf8_error(&format!(
                "Contains potentially confusing Unicode character: U+{:04X}",
                ch as u32
            )));
        }
    }

    Ok(())
}

fn is_confusing_unicode(ch: char) -> bool {
    // Check for characters that might be used for visual spoofing
    match ch {
        // Zero-width characters
        '\u{200B}' | '\u{200C}' | '\u{200D}' | '\u{FEFF}' => true,
        // Right-to-left override characters
        '\u{202D}' | '\u{202E}' => true,
        // Other potentially confusing characters
        '\u{00A0}' => true, // Non-breaking space
        _ => false,
    }
}

pub(crate) fn validate_input_structure(input: &str) -> Result<(), TranspileError> {
    // Check for balanced parentheses, brackets, and braces
    let mut paren_count = 0;
    let mut bracket_count = 0;
    let mut brace_count = 0;
    let mut in_string = false;
    let mut escape_next = false;
    let mut string_char = '\0';

    for ch in input.chars() {
        if escape_next {
            escape_next = false;
            continue;
        }

        if ch == '\\' {
            escape_next = true;
            continue;
        }

        if in_string {
            if ch == string_char {
                in_string = false;
            }
            continue;
        }

        match ch {
            '"' | '\'' => {
                in_string = true;
                string_char = ch;
            }
            '(' => paren_count += 1,
            ')' => {
                paren_count -= 1;
                if paren_count < 0 {
                    return Err(TranspileError::syntax_error_with_suggestion(
                        "Unmatched closing parenthesis",
                        0, // Position tracking would require more complex parsing
                        Some(")".to_string()),
                        Some("Check parentheses balance".to_string()),
                    ));
                }
            }
            '[' => bracket_count += 1,
            ']' => {
                bracket_count -= 1;
                if bracket_count < 0 {
                    return Err(TranspileError::syntax_error_with_suggestion(
                        "Unmatched closing bracket",
                        0,
                        Some("]".to_string()),
                        Some("Check brackets balance".to_string()),
                    ));
                }
            }
            '{' => brace_count += 1,
            '}' => {
                brace_count -= 1;
                if brace_count < 0 {
                    return Err(TranspileError::syntax_error_with_suggestion(
                        "Unmatched closing brace",
                        0,
                        Some("}".to_string()),
                        Some("Check braces balance".to_string()),
                    ));
                }
            }
            _ => {}
        }
    }

    // Check for unclosed delimiters
    if paren_count > 0 {
        return Err(TranspileError::syntax_error_with_suggestion(
            &format!("{} unclosed parentheses", paren_count),
            0,
            Some("(".to_string()),
            Some("Add missing closing parentheses".to_string()),
        ));
    }

    if bracket_count > 0 {
        return Err(TranspileError::syntax_error_with_suggestion(
            &format!("{} unclosed brackets", bracket_count),
            0,
            Some("[".to_string()),
            Some("Add missing closing brackets".to_string()),
        ));
    }

    if brace_count > 0 {
        return Err(TranspileError::syntax_error_with_suggestion(
            &format!("{} unclosed braces", brace_count),
            0,
            Some("{".to_string()),
            Some("Add missing closing braces".to_string()),
        ));
    }

    if in_string {
        return Err(TranspileError::syntax_error_with_suggestion(
            "Unclosed string literal",
            0,
            Some(string_char.to_string()),
            Some("Add missing closing quote".to_string()),
        ));
    }

    Ok(())
}
