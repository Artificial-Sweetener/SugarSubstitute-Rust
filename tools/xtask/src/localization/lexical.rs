// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Identify native presentation and script properties without promoting collection operations.
use super::sink;
use crate::source_tokens::{self, Token};
use std::collections::BTreeSet;

/// Browser setters belong to JavaScript and embedded HTML/SVG script surfaces.
pub(super) fn dom_language(extension: &str) -> bool {
    matches!(
        extension.to_ascii_lowercase().as_str(),
        "js" | "jsx" | "mjs" | "cjs" | "ts" | "tsx" | "mts" | "cts" | "html" | "svg"
    )
}
fn dom_only(name: &str) -> bool {
    matches!(
        name,
        "append"
            | "appendChild"
            | "replaceChildren"
            | "insertAdjacentText"
            | "Option"
            | "Notification"
            | "Reflect"
            | "defineProperty"
            | "defineProperties"
            | "__defineSetter__"
            | "alert"
            | "confirm"
            | "prompt"
            | "textContent"
            | "innerText"
            | "innerHTML"
            | "outerHTML"
            | "insertAdjacentHTML"
            | "setAttribute"
            | "createTextNode"
            | "document.write"
    )
}
/// A data parameter or a protocol opcode named append does not select the DOM method.
fn selected(tokens: &[Token], index: usize) -> bool {
    matches!(
        index.checked_sub(1).and_then(|before| tokens.get(before)),
        Some(Token::Symbol('.' | '['))
    ) || tokens[..index].iter().rposition(|token| *token == Token::Symbol('{')).is_some_and(
        |start| {
            matches!(
                start.checked_sub(1).and_then(|before| tokens.get(before)).and_then(Token::text),
                Some("const" | "let" | "var" | "import")
            )
        },
    )
}
fn admitted(name: &str, dom: bool, member: bool) -> bool {
    sink(name) && (!dom_only(name) || dom) && (name != "append" || member)
}
/// Inspect decoded names, actual member selection and adjacent quoted concatenations.
/// Native/Qt names remain governed independently of browser-only collection method spellings.
pub(super) fn findings(source: &str, extension: &str) -> Vec<String> {
    let tokens = source_tokens::lex(source);
    let dom = dom_language(extension);
    let mut findings = BTreeSet::new();
    let mut names = BTreeSet::new();
    for (index, token) in tokens.iter().enumerate() {
        if let Some(text) = token.text() {
            for name in text.split(|c: char| !c.is_alphanumeric() && c != '_' && c != '$') {
                names.insert(name.to_owned());
                if admitted(name, dom, selected(&tokens, index)) {
                    findings.insert(name.to_owned());
                }
            }
        }
        if let Token::Literal(first) = token {
            let mut name = first.clone();
            for next in [index + 2, index + 4] {
                let Some(Token::Symbol('+')) = tokens.get(next - 1) else { break };
                let Some(Token::Literal(part)) = tokens.get(next) else { break };
                name.push_str(part);
                if admitted(&name, dom, selected(&tokens, index)) {
                    findings.insert(name.clone());
                }
            }
        }
    }
    for name in
        ["LOCALES", "SUPPORTED_LOCALES", "supportedLocales", "i18n", "statusCode", "textOwner"]
    {
        if names.contains(name) {
            findings.insert(format!("registry access: {name}"));
        }
    }
    findings.into_iter().collect()
}
