use crate::folding_ranges::folding_ranges;

/// Helper: find a folding range whose `startLine`/`endLine` cover the given
/// half-open line range, optionally matching a `kind`.
fn find_range<'a>(
    ranges: &'a [serde_json::Value],
    start_line: u64,
    end_line: u64,
    kind: &'a str,
) -> Option<&'a serde_json::Value> {
    ranges.iter().find(|r| {
        r["startLine"].as_u64() == Some(start_line)
            && r["endLine"].as_u64() == Some(end_line)
            && r["kind"].as_str() == Some(kind)
    })
}

/// Helper: does any range have the given kind?
fn has_kind(ranges: &[serde_json::Value], kind: &str) -> bool {
    ranges.iter().any(|r| r["kind"].as_str() == Some(kind))
}

#[test]
fn folding_ranges_for_loop_block() {
    // The `loop {` opens on line 0 and closes on line 3.
    let src = "\
loop {
  remaining: input,
  acc:       []
} (
  state ~> match state.remaining {
    []: returnA -< {
      out: state.acc
    }
  }
)
";
    let ranges = folding_ranges(src);
    // The outer `{ ... }` of the loop spans lines 0..=3.
    assert!(
        find_range(&ranges, 0, 3, "region").is_some(),
        "expected a region folding range for the loop body covering lines 0..=3, got: {ranges:?}"
    );
}

#[test]
fn folding_ranges_for_arrow_block() {
    // The `state ~> match ... {` body opens on line 4 and closes on
    // line 8.
    let src = "\
loop {
  remaining: input,
  acc:       []
} (
  state ~> match state.remaining {
    []: returnA -< {
      out: state.acc
    }
  }
)
";
    let ranges = folding_ranges(src);
    // The match body `{ ... }` spans lines 4..=8.
    assert!(
        find_range(&ranges, 4, 8, "region").is_some(),
        "expected a region folding range for the arrow/match body covering lines 4..=8, got: {ranges:?}"
    );
}

#[test]
fn folding_ranges_for_match_block() {
    // Two match arms, each with a multi-line object body.
    let src = "\
match state.remaining {
  []: {
    out: state.acc
  },

  [h, ...t]: {
    other: state.acc
  }
}
";
    let ranges = folding_ranges(src);
    // The outer match body `{ ... }` spans lines 0..=8.
    assert!(
        find_range(&ranges, 0, 8, "region").is_some(),
        "expected a region folding range for the match body, got: {ranges:?}"
    );
    // The first arm body `{ ... }` spans lines 1..=3.
    assert!(
        find_range(&ranges, 1, 3, "region").is_some(),
        "expected a region folding range for the first match arm, got: {ranges:?}"
    );
    // The second arm body `{ ... }` spans lines 5..=7.
    assert!(
        find_range(&ranges, 5, 7, "region").is_some(),
        "expected a region folding range for the second match arm, got: {ranges:?}"
    );
}

#[test]
fn folding_ranges_for_object_literal() {
    // `{ out: state.acc }` spans two lines.
    let src = "\
returnA -< {
  out: state.acc
}
";
    let ranges = folding_ranges(src);
    assert!(
        find_range(&ranges, 0, 2, "region").is_some(),
        "expected a region folding range for the object literal, got: {ranges:?}"
    );
}

#[test]
fn folding_ranges_for_doc_comments() {
    // A run of three doc-comment lines at the top of the file.
    let src = "\
--- sort: insertion sort over a list.
---
--- This is the second line of the doc comment.
loop {
  acc: []
}
";
    let ranges = folding_ranges(src);
    assert!(
        has_kind(&ranges, "comment"),
        "expected a `comment` folding range for the doc comment block, got: {ranges:?}"
    );
    // The doc comment block spans lines 0..=2.
    assert!(
        find_range(&ranges, 0, 2, "comment").is_some(),
        "expected a comment folding range covering lines 0..=2, got: {ranges:?}"
    );
}

#[test]
fn folding_ranges_single_line_no_range() {
    // A single-line `{ ... }` should not produce a folding range.
    let src = "loop { acc: [] }";
    let ranges = folding_ranges(src);
    assert!(
        ranges.iter().all(|r| r["kind"].as_str() != Some("region")
            || r["startLine"].as_u64() != r["endLine"].as_u64()),
        "single-line braces should not produce a folding range, got: {ranges:?}"
    );
    // More directly: there should be no region range at all here.
    assert!(
        !has_kind(&ranges, "region"),
        "single-line `{{ ... }}` must not produce a region folding range, got: {ranges:?}"
    );
}

#[test]
fn folding_ranges_empty_source_returns_empty() {
    let ranges = folding_ranges("");
    assert!(
        ranges.is_empty(),
        "empty source should produce no folding ranges, got: {ranges:?}"
    );
}

#[test]
fn folding_ranges_returns_valid_json() {
    let src = "\
/// doc comment line one
/// doc comment line two
loop {
  remaining: input,
  acc:       []
} (
  state ~> match state.remaining {
    []: returnA -< {
      out: state.acc
    }
  }
)
";
    let ranges = folding_ranges(src);
    assert!(
        !ranges.is_empty(),
        "expected some folding ranges for a non-trivial source, got: {ranges:?}"
    );
    for r in &ranges {
        assert!(
            r.get("startLine").and_then(|v| v.as_u64()).is_some(),
            "range missing numeric startLine: {r:?}"
        );
        assert!(
            r.get("endLine").and_then(|v| v.as_u64()).is_some(),
            "range missing numeric endLine: {r:?}"
        );
        assert!(
            r.get("kind").and_then(|v| v.as_str()).is_some(),
            "range missing string kind: {r:?}"
        );
        let kind = r["kind"].as_str().unwrap();
        assert!(
            kind == "region" || kind == "comment",
            "range kind should be 'region' or 'comment', got {kind:?}"
        );
    }

    // Ranges should be sorted by startLine.
    let mut prev: i64 = -1;
    for r in &ranges {
        let line = r["startLine"].as_u64().unwrap() as i64;
        assert!(
            line >= prev,
            "folding ranges should be sorted by startLine, got: {ranges:?}"
        );
        prev = line;
    }
}

#[test]
#[ignore = "perf guard: run with --ignored; flaky under parallel test contention"]
fn folding_ranges_performance_on_large_file() {
    let mut src = String::from("--- Large benchmark file\n");
    for i in 0..500 {
        src.push_str(&format!("arrow_{} = arr(\"op_{}\") >>> first(id)\n", i, i));
    }
    src.push_str("\nloop {\n  remaining: input,\n  acc: []\n} (\n");
    for i in 0..200 {
        src.push_str(&format!("  [h, ...t]: do {{\n    r{} <- arrow_{} -< {{ elem: h, acc: state.acc }};\n    returnA -< {{\n      state: {{ remaining: t, acc: r{} }}\n    }}\n  }}\n", i, i % 500, i));
    }
    src.push_str(")\n");

    // Warm up (populate caches / allocator) then take the best of a few
    // runs so the assertion is robust against parallel-test contention.
    let _ = folding_ranges(&src);
    let mut best = std::time::Duration::MAX;
    for _ in 0..5 {
        let start = std::time::Instant::now();
        let _ = folding_ranges(&src);
        best = best.min(start.elapsed());
    }
    assert!(best.as_millis() < 50, "fold too slow: {:?}", best);
}

