// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Reject authored visible HTML copy outside catalog slots and immutable product identity.
use crate::error::{Result, require};

/// Inspect markup text/accessible labels; script/style bodies are handled by capability inspection.
pub(super) fn findings(source: &str) -> Result<Vec<String>> {
    let mut input = source;
    let mut findings = Vec::new();
    while !input.is_empty() {
        let next = input.find('<').unwrap_or(input.len());
        if owned(&input[..next]) {
            findings.push("owned HTML text literal".into());
        }
        input = &input[next..];
        if input.is_empty() {
            break;
        }
        if let Some(comment) = input.strip_prefix("<!--") {
            let end = comment.find("-->");
            require(end.is_some(), "localization: unterminated HTML comment")?;
            input = &comment[end.unwrap_or(0) + 3..];
            continue;
        }
        let end = input.find('>');
        require(end.is_some(), "localization: unterminated HTML tag")?;
        let end = end.unwrap_or(0);
        let tag = &input[1..end];
        input = &input[end + 1..];
        let name = tag.split_whitespace().next().unwrap_or("").to_ascii_lowercase();
        let attributes = attributes(tag);
        let button_value = name == "button"
            || (name == "input"
                && attributes.iter().any(|(key, value)| {
                    key.eq_ignore_ascii_case("type")
                        && matches!(
                            value.to_ascii_lowercase().as_str(),
                            "button" | "submit" | "reset"
                        )
                }));
        for (key, value) in attributes {
            let key = key.to_ascii_lowercase();
            if (matches!(key.as_str(), "title" | "alt" | "aria-label" | "placeholder")
                || (key == "value" && button_value))
                && owned(value)
            {
                findings.push(format!("owned HTML {key} literal"));
            }
        }
        if matches!(name.as_str(), "script" | "style") {
            let closing = format!("</{name}");
            let lower = input.to_ascii_lowercase();
            let end = lower.find(&closing);
            require(end.is_some(), "localization: unterminated raw HTML body")?;
            input = &input[end.unwrap_or(0)..];
        }
    }
    findings.sort();
    findings.dedup();
    Ok(findings)
}

/// Read quoted or unquoted attribute values with HTML's case-insensitive names and optional spacing.
fn attributes(tag: &str) -> Vec<(&str, &str)> {
    let mut rest = tag.trim_start();
    rest = &rest[rest.find(char::is_whitespace).unwrap_or(rest.len())..];
    let mut attributes = Vec::new();
    loop {
        rest = rest.trim_start();
        if rest.is_empty() || rest.starts_with('/') {
            break;
        }
        let end = rest.find(|c: char| c.is_whitespace() || c == '=').unwrap_or(rest.len());
        if end == 0 {
            break;
        }
        let key = &rest[..end];
        rest = rest[end..].trim_start();
        let Some(value) = rest.strip_prefix('=') else {
            continue;
        };
        rest = value.trim_start();
        let quote = rest.chars().next().unwrap_or(' ');
        let value = if matches!(quote, '\'' | '"') {
            rest = &rest[1..];
            let end = rest.find(quote).unwrap_or(rest.len());
            let value = &rest[..end];
            rest = if end < rest.len() { &rest[end + 1..] } else { "" };
            value
        } else {
            let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
            let value = &rest[..end];
            rest = &rest[end..];
            value
        };
        attributes.push((key, value));
    }
    attributes
}

/// Slots contain catalog or separately registered asset data, never surrounding authored prose.
fn owned(text: &str) -> bool {
    let text = text.trim();
    if text.is_empty() || text == "SugarSubstitute" {
        return false;
    }
    !(text.len() > 4
        && text.starts_with("__")
        && text.ends_with("__")
        && text[2..text.len() - 2].chars().all(|c| c.is_ascii_uppercase() || c == '_'))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn visible_and_accessible_copy_cannot_bypass_the_catalog_with_markup() -> Result<()> {
        for source in [
            "<h1>English owned copy</h1>",
            "<input TYPE = 'submit' VALUE = 'English'>",
            "<button TITLE = 'English'></button>",
            "<button title='English'>__TITLE__</button>",
            "<input placeholder=English>",
            "<p>prefix __TITLE__</p>",
            "<svg><title>English logo</title><text>English status</text></svg>",
        ] {
            assert!(!findings(source)?.is_empty(), "accepted {source}");
        }
        assert!(findings("<!-- preserved source attribution --><html lang='__LOCALE__'><title>__TITLE__</title><div aria-label='SugarSubstitute'>__WORDMARK__</div><script>const diagnostic='English';</script></html>")?.is_empty());
        for source in ["<!-- incomplete", "<script>missing", "<h1"] {
            assert!(findings(source).is_err());
        }
        Ok(())
    }
}
