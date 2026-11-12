// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Measure Rust source through token locations so literals cannot hide structural growth.

use std::collections::BTreeSet;

use proc_macro2::{Span, TokenStream, TokenTree};

use crate::error::{Failure, Result};

/// Count nonblank code lines while excluding comments and retaining literal contents.
pub(crate) fn code_lines(source: &str) -> Result<usize> {
    let tokens: TokenStream =
        source.parse().map_err(|error| Failure::external("tokenize structural source", error))?;
    let lines: Vec<_> = source.lines().collect();
    let mut occupied = BTreeSet::new();
    collect(tokens, &lines, &mut occupied);
    Ok(occupied.len())
}

/// Retain delimiter and token locations without counting comments inside groups.
fn collect(tokens: TokenStream, lines: &[&str], occupied: &mut BTreeSet<usize>) {
    for token in tokens {
        match token {
            TokenTree::Group(group) => {
                if comment_origin(group.span(), lines) {
                    continue;
                }
                occupy(group.span_open(), lines, occupied);
                collect(group.stream(), lines, occupied);
                occupy(group.span_close(), lines, occupied);
            }
            token => occupy(token.span(), lines, occupied),
        }
    }
}

/// Exclude synthesized doc-comment tokens using their original source position.
fn occupy(span: Span, lines: &[&str], occupied: &mut BTreeSet<usize>) {
    let start = span.start();
    let end = span.end();
    if comment_origin(span, lines) {
        return;
    }
    for number in start.line..=end.line {
        if lines.get(number.saturating_sub(1)).is_some_and(|line| !line.trim().is_empty()) {
            occupied.insert(number);
        }
    }
}

/// Identify attribute tokens synthesized by Rust's doc-comment desugaring.
fn comment_origin(span: Span, lines: &[&str]) -> bool {
    let start = span.start();
    let origin = lines
        .get(start.line.saturating_sub(1))
        .and_then(|line| line.get(start.column..))
        .unwrap_or_default();
    origin.starts_with("//") || origin.starts_with("/*")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Exclude nested comments and Rustdoc while retaining code after a comment.
    #[test]
    fn measurement_excludes_comments_without_hiding_code() -> Result<()> {
        assert_eq!(
            code_lines(
                "//! module docs\n/// item docs\n/* outer\n/* nested */\n*/ fn a() {}\n\nlet x = 1; // value"
            )?,
            2
        );
        assert_eq!(code_lines("/** multiline\n * docs\n */\nfn a() {}")?, 1);
        assert_eq!(code_lines("fn first() {} /** more\n * docs\n */ fn second() {}")?, 2);
        Ok(())
    }

    /// Count raw literal lines that resemble comments and preserve Unicode positions.
    #[test]
    fn measurement_retains_multiline_literal_contents() -> Result<()> {
        assert_eq!(
            code_lines("let text = r#\"\n// literal content\n/* literal content */\n\"#;")?,
            4
        );
        assert_eq!(code_lines("let 中文 = \"值\";\n// comment\nfn a() {}")?, 2);
        assert!(code_lines("fn broken( {").is_err());
        Ok(())
    }
}
