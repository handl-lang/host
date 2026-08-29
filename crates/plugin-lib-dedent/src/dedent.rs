//! Strips the common leading whitespace from every line in `text`.
//!
//! - Lines that are empty or contain only whitespace are ignored when
//!   computing the common indentation.
//! - The first and trailing newlines are trimmed.
//! - Tab and space characters are both counted as whitespace.
pub fn dedent(text: &str) -> String {
    let trimmed = text.trim_matches('\n');

    if trimmed.is_empty() {
        return String::new();
    }

    let common_indent = trimmed
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let mut count = 0usize;
            for ch in line.chars() {
                if ch == ' ' || ch == '\t' {
                    count += 1;
                } else {
                    break;
                }
            }
            count
        })
        .min()
        .unwrap_or(0);

    trimmed
        .lines()
        .map(|line| {
            if line.len() >= common_indent
                && line.chars().take(common_indent).all(|c| c == ' ' || c == '\t')
            {
                &line[common_indent..]
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}
