use crate::number_format::*;
use ecma402_traits::number_format::{NumberFormatPart, NumberFormatPartType};

fn np(t: NumberFormatPartType, v: &str) -> NumberFormatPart {
    NumberFormatPart {
        part_type: t,
        value: v.to_string(),
    }
}

#[test]
fn split_percent_parts_integer_then_sign() {
    assert_eq!(
        split_percent_parts("5%", &[np(NumberFormatPartType::Integer, "5")]),
        vec![
            np(NumberFormatPartType::Integer, "5"),
            np(NumberFormatPartType::PercentSign, "%"),
        ]
    );
}

#[test]
fn split_percent_parts_grouped_integer() {
    assert_eq!(
        split_percent_parts("100%", &[np(NumberFormatPartType::Integer, "100")]),
        vec![
            np(NumberFormatPartType::Integer, "100"),
            np(NumberFormatPartType::PercentSign, "%"),
        ]
    );
}

#[test]
fn split_percent_parts_with_fraction() {
    assert_eq!(
        split_percent_parts(
            "12.50%",
            &[
                np(NumberFormatPartType::Integer, "12"),
                np(NumberFormatPartType::Decimal, "."),
                np(NumberFormatPartType::Fraction, "50"),
            ]
        ),
        vec![
            np(NumberFormatPartType::Integer, "12"),
            np(NumberFormatPartType::Decimal, "."),
            np(NumberFormatPartType::Fraction, "50"),
            np(NumberFormatPartType::PercentSign, "%"),
        ]
    );
}

#[test]
fn split_percent_parts_whitespace_gap_is_literal() {
    assert_eq!(
        split_percent_parts("100 %", &[np(NumberFormatPartType::Integer, "100")]),
        vec![
            np(NumberFormatPartType::Integer, "100"),
            np(NumberFormatPartType::Literal, " "),
            np(NumberFormatPartType::PercentSign, "%"),
        ]
    );
}

#[test]
fn split_percent_parts_no_number_parts() {
    assert_eq!(
        split_percent_parts("%", &[]),
        vec![np(NumberFormatPartType::PercentSign, "%")]
    );
}

#[test]
fn split_percent_parts_leading_sign() {
    assert_eq!(
        split_percent_parts(
            "-5%",
            &[
                np(NumberFormatPartType::MinusSign, "-"),
                np(NumberFormatPartType::Integer, "5"),
            ]
        ),
        vec![
            np(NumberFormatPartType::MinusSign, "-"),
            np(NumberFormatPartType::Integer, "5"),
            np(NumberFormatPartType::PercentSign, "%"),
        ]
    );
}

#[test]
fn relabel_currency_parts_symbol_prefix() {
    assert_eq!(
        relabel_currency_parts(vec![
            np(NumberFormatPartType::Literal, "$"),
            np(NumberFormatPartType::Integer, "1"),
            np(NumberFormatPartType::Group, ","),
            np(NumberFormatPartType::Integer, "234"),
            np(NumberFormatPartType::Decimal, "."),
            np(NumberFormatPartType::Fraction, "50"),
        ]),
        vec![
            np(NumberFormatPartType::Currency, "$"),
            np(NumberFormatPartType::Integer, "1"),
            np(NumberFormatPartType::Group, ","),
            np(NumberFormatPartType::Integer, "234"),
            np(NumberFormatPartType::Decimal, "."),
            np(NumberFormatPartType::Fraction, "50"),
        ]
    );
}

#[test]
fn relabel_currency_parts_keeps_whitespace_literal() {
    assert_eq!(
        relabel_currency_parts(vec![
            np(NumberFormatPartType::Integer, "1"),
            np(NumberFormatPartType::Group, ","),
            np(NumberFormatPartType::Integer, "234"),
            np(NumberFormatPartType::Decimal, "."),
            np(NumberFormatPartType::Fraction, "50"),
            np(NumberFormatPartType::Literal, " "),
            np(NumberFormatPartType::Literal, "US dollars"),
        ]),
        vec![
            np(NumberFormatPartType::Integer, "1"),
            np(NumberFormatPartType::Group, ","),
            np(NumberFormatPartType::Integer, "234"),
            np(NumberFormatPartType::Decimal, "."),
            np(NumberFormatPartType::Fraction, "50"),
            np(NumberFormatPartType::Literal, " "),
            np(NumberFormatPartType::Currency, "US dollars"),
        ]
    );
}

#[test]
fn relabel_currency_parts_no_literals() {
    let parts = vec![
        np(NumberFormatPartType::Integer, "5"),
        np(NumberFormatPartType::Decimal, "."),
        np(NumberFormatPartType::Fraction, "00"),
    ];
    assert_eq!(relabel_currency_parts(parts.clone()), parts);
}
