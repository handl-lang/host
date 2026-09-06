use serde_json::json;

use super::color_provider::{color_presentations, document_colors};

#[test]
fn document_colors_finds_hex_6_digit() {
    let source = "let bg = #ff0000";
    let colors = document_colors(source, None);
    assert_eq!(colors.len(), 1);
    let color = &colors[0]["color"];
    assert_eq!(color["red"].as_f64().unwrap(), 1.0);
    assert_eq!(color["green"].as_f64().unwrap(), 0.0);
    assert_eq!(color["blue"].as_f64().unwrap(), 0.0);
    assert_eq!(color["alpha"].as_f64().unwrap(), 1.0);
    // Range covers the 7 characters `#ff0000` on line 0.
    let range = &colors[0]["range"];
    assert_eq!(range["start"]["line"].as_u64().unwrap(), 0);
    assert_eq!(range["start"]["character"].as_u64().unwrap(), 9);
    assert_eq!(range["end"]["character"].as_u64().unwrap(), 16);
}

#[test]
fn document_colors_finds_hex_3_digit() {
    let source = "let bg = #f00";
    let colors = document_colors(source, None);
    assert_eq!(colors.len(), 1);
    let color = &colors[0]["color"];
    // #f00 expands to #ff0000.
    assert_eq!(color["red"].as_f64().unwrap(), 1.0);
    assert_eq!(color["green"].as_f64().unwrap(), 0.0);
    assert_eq!(color["blue"].as_f64().unwrap(), 0.0);
    let range = &colors[0]["range"];
    assert_eq!(range["start"]["character"].as_u64().unwrap(), 9);
    assert_eq!(range["end"]["character"].as_u64().unwrap(), 13);
}

#[test]
fn document_colors_finds_named_color() {
    let source = "let bg = color(\"red\")";
    let colors = document_colors(source, None);
    assert_eq!(colors.len(), 1);
    let color = &colors[0]["color"];
    assert_eq!(color["red"].as_f64().unwrap(), 1.0);
    assert_eq!(color["green"].as_f64().unwrap(), 0.0);
    assert_eq!(color["blue"].as_f64().unwrap(), 0.0);
    assert_eq!(color["alpha"].as_f64().unwrap(), 1.0);
    // Range covers the string literal `"red"` (5 characters) starting at 15.
    let range = &colors[0]["range"];
    assert_eq!(range["start"]["line"].as_u64().unwrap(), 0);
    assert_eq!(range["start"]["character"].as_u64().unwrap(), 15);
    assert_eq!(range["end"]["character"].as_u64().unwrap(), 20);
}

#[test]
fn document_colors_finds_multiple_colors() {
    let source = "let a = #ff0000\nlet b = color(\"blue\")\nlet c = #0f0";
    let colors = document_colors(source, None);
    assert_eq!(colors.len(), 3);
    // Hex literals are collected first (in source order), then named colors.
    // First: #ff0000 (red).
    assert_eq!(colors[0]["color"]["red"].as_f64().unwrap(), 1.0);
    // Second: #0f0 (green).
    assert_eq!(colors[1]["color"]["green"].as_f64().unwrap(), 1.0);
    // Third: color("blue").
    assert_eq!(colors[2]["color"]["blue"].as_f64().unwrap(), 1.0);
}

#[test]
fn document_colors_empty_source_returns_empty() {
    let colors = document_colors("", None);
    assert!(colors.is_empty());
}

#[test]
fn document_colors_no_colors_returns_empty() {
    let source = "let x = 1 + 2\nprint(x)";
    let colors = document_colors(source, None);
    assert!(colors.is_empty());
}

#[test]
fn color_presentations_returns_hex_and_rgb() {
    let color = json!({ "red": 1.0, "green": 0.0, "blue": 0.0, "alpha": 1.0 });
    let range = json!({
        "start": { "line": 0, "character": 0 },
        "end": { "line": 0, "character": 7 },
    });
    let presentations = color_presentations(&color, &range);
    assert_eq!(presentations.len(), 2);
    let labels: Vec<String> = presentations
        .iter()
        .map(|p| p["label"].as_str().unwrap().to_string())
        .collect();
    assert!(labels.contains(&"#ff0000".to_string()));
    assert!(labels.contains(&"rgb(255, 0, 0)".to_string()));
}

#[test]
fn color_presentations_for_black() {
    let color = json!({ "red": 0.0, "green": 0.0, "blue": 0.0, "alpha": 1.0 });
    let range = json!({
        "start": { "line": 0, "character": 0 },
        "end": { "line": 0, "character": 7 },
    });
    let presentations = color_presentations(&color, &range);
    assert_eq!(presentations.len(), 2);
    let labels: Vec<String> = presentations
        .iter()
        .map(|p| p["label"].as_str().unwrap().to_string())
        .collect();
    assert!(labels.contains(&"#000000".to_string()));
    assert!(labels.contains(&"rgb(0, 0, 0)".to_string()));
}
