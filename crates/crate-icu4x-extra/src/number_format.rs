use ecma402_traits::number_format::{NumberFormatPart, NumberFormatPartType};

/// Splits a percent-formatted `formatted` string into spec-aligned
/// `NumberFormatPart`s by locating the `number_parts` within it.
///
/// `number_parts` should be produced by formatting the percent value (the
/// input multiplied by 100) with a decimal `NumberFormat` using the same
/// digit/sign options. ICU4X 2.x's `PercentFormatter` does not emit
/// `percentSign` part annotations — it writes the whole formatted value as a
/// single `literal` — so this helper recovers the `integer`/`fraction`/`group`/
/// `decimal` parts from `number_parts` and classifies the remaining text:
/// non-whitespace runs become `percentSign` parts and whitespace runs become
/// `literal` parts.
pub fn split_percent_parts(
    formatted: &str,
    number_parts: &[NumberFormatPart],
) -> Vec<NumberFormatPart> {
    split_parts(formatted, number_parts, |gap| {
        let mut parts = Vec::new();
        let mut start = 0;
        let bytes = gap.as_bytes();
        while start < bytes.len() {
            let is_ws = bytes[start].is_ascii_whitespace();
            let mut end = start + 1;
            while end < bytes.len() && bytes[end].is_ascii_whitespace() == is_ws {
                end += 1;
            }
            parts.push(NumberFormatPart {
                part_type: if is_ws {
                    NumberFormatPartType::Literal
                } else {
                    NumberFormatPartType::PercentSign
                },
                value: gap[start..end].to_string(),
            });
            start = end;
        }
        parts
    })
}

/// Relabels `literal` parts emitted by ICU4X 2.x's `CurrencyFormatter` as
/// `currency` parts.
///
/// The currency formatter emits the currency token (symbol, code, or name) as
/// an undifferentiated `literal` while the numeric parts (`integer`, `group`,
/// `decimal`, `fraction`, signs) come through correctly. This relabels
/// non-whitespace `literal` parts as `currency` and leaves whitespace
/// separators as `literal`.
pub fn relabel_currency_parts(parts: Vec<NumberFormatPart>) -> Vec<NumberFormatPart> {
    parts
        .into_iter()
        .map(|p| {
            if p.part_type == NumberFormatPartType::Literal && !p.value.trim().is_empty() {
                NumberFormatPart {
                    part_type: NumberFormatPartType::Currency,
                    value: p.value,
                }
            } else {
                p
            }
        })
        .collect()
}

/// Locates the `number_parts`' values (in order) within `formatted` and emits
/// them with their original part types, passing the text between/around them
/// through `classify_gap` to split each gap into one or more parts.
fn split_parts(
    formatted: &str,
    number_parts: &[NumberFormatPart],
    mut classify_gap: impl FnMut(&str) -> Vec<NumberFormatPart>,
) -> Vec<NumberFormatPart> {
    let mut result = Vec::new();
    let mut remaining = formatted;

    for np in number_parts {
        if np.value.is_empty() {
            continue;
        }
        match remaining.find(&np.value) {
            Some(pos) => {
                let gap = &remaining[..pos];
                if !gap.is_empty() {
                    result.extend(classify_gap(gap));
                }
                result.push(NumberFormatPart {
                    part_type: np.part_type.clone(),
                    value: np.value.clone(),
                });
                remaining = &remaining[pos + np.value.len()..];
            }
            None => {
                // The formatted pattern may omit some number parts (e.g. a
                // leading zero trimmed by the percent formatter); keep looking
                // for later parts.
            }
        }
    }

    if !remaining.is_empty() {
        result.extend(classify_gap(remaining));
    }

    result
}
