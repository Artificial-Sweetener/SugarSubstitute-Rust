// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Derive gate identities from the actual registry instead of duplicating supported languages.
use crate::error::{Failure, Result, require};
use std::collections::BTreeSet;
use syn::visit::Visit;

#[derive(Default)]
struct Identities(Vec<String>);
impl<'ast> Visit<'ast> for Identities {
    fn visit_expr_struct(&mut self, expression: &'ast syn::ExprStruct) {
        if expression.path.segments.last().is_some_and(|name| name.ident == "Messages") {
            for field in &expression.fields {
                if matches!(&field.member, syn::Member::Named(name) if name == "id")
                    && let syn::Expr::Lit(literal) = &field.expr
                    && let syn::Lit::Str(text) = &literal.lit
                {
                    self.0.push(text.value());
                }
            }
        }
        syn::visit::visit_expr_struct(self, expression);
    }
}
/// Require a canonical English-first registry, unique explicit ids, and exact dynamic gate inputs.
pub(super) fn identities(source: &str) -> Result<BTreeSet<String>> {
    let file = syn::parse_file(source)
        .map_err(|cause| Failure::external("parse locale registry", cause))?;
    let mut identities = Identities::default();
    identities.visit_file(&file);
    let set: BTreeSet<_> = identities.0.iter().cloned().collect();
    require(
        identities.0.first().is_some_and(|id| id == "en")
            && set.len() == identities.0.len()
            && set.iter().all(|id| {
                !id.is_empty()
                    && id.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
            }),
        "localization: registry requires canonical English first and unique explicit locale identities",
    )?;
    Ok(set)
}

/// Primitive collections of known ids are a second inventory; independent diagnostic strings are not.
/// This lexer preserves comments/quoted text and does not claim to evaluate arbitrary script execution.
pub(super) fn parallel_literals(source: &str, locales: &BTreeSet<String>) -> bool {
    struct Collection {
        closing: char,
        inventory: bool,
        values: BTreeSet<String>,
    }
    let text: Vec<_> = source.chars().collect();
    let mut index = 0;
    let mut previous = String::new();
    let mut stack: Vec<Collection> = Vec::new();
    while index < text.len() {
        let character = text[index];
        if character.is_whitespace() {
            index += 1;
            continue;
        }
        if character == '/' && text.get(index + 1) == Some(&'/') {
            while index < text.len() && text[index] != '\n' {
                index += 1;
            }
            continue;
        }
        if character == '/' && text.get(index + 1) == Some(&'*') {
            index += 2;
            while index + 1 < text.len() && !(text[index] == '*' && text[index + 1] == '/') {
                index += 1;
            }
            index = (index + 2).min(text.len());
            continue;
        }
        if matches!(character, '\'' | '"' | '`') {
            let quote = character;
            index += 1;
            let mut value = String::new();
            while index < text.len() {
                let character = text[index];
                index += 1;
                if character == quote {
                    break;
                }
                if character == '\\' {
                    value.push(character);
                    if let Some(character) = text.get(index) {
                        value.push(*character);
                        index += 1;
                    }
                } else {
                    value.push(character);
                }
            }
            if locales.contains(&value)
                && let Some(collection) = stack.last_mut()
                && collection.inventory
            {
                collection.values.insert(value);
            }
            previous = "literal".into();
            continue;
        }
        if matches!(character, '[' | '{') {
            let inventory = character == '['
                || matches!(previous.as_str(), "=" | ":" | "," | "[" | "(" | "return" | "default");
            stack.push(Collection {
                closing: if character == '[' { ']' } else { '}' },
                inventory,
                values: BTreeSet::new(),
            });
        } else if matches!(character, ']' | '}') {
            if let Some(collection) = stack.pop()
                && collection.closing == character
                && collection.inventory
            {
                if collection.values.len() > 1 {
                    return true;
                }
                if let Some(parent) = stack.last_mut()
                    && parent.inventory
                {
                    parent.values.extend(collection.values);
                }
            }
        } else if character.is_alphabetic() || character == '_' {
            let start = index;
            while index < text.len()
                && (text[index].is_alphanumeric() || matches!(text[index], '_' | '$'))
            {
                index += 1;
            }
            let word: String = text[start..index].iter().collect();
            let next = text[index..].iter().find(|character| !character.is_whitespace());
            if next == Some(&':')
                && locales.contains(&word)
                && let Some(collection) = stack.last_mut()
                && collection.inventory
            {
                collection.values.insert(word.clone());
            }
            previous = word;
            continue;
        }
        previous = character.to_string();
        index += 1;
    }
    false
}

#[cfg(test)]
mod tests;
