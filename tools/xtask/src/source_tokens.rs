// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Tokenize conservative authored script/native identities and decoded quoted properties.
/// Ignore comments, decode quoted escapes and retain identifiers and computed string properties.
pub(crate) fn tokens(source: &str) -> Vec<String> {
    let mut chars = source.chars().peekable();
    let mut output = Vec::new();
    while let Some(character) = chars.next() {
        if character == '/' && chars.peek() == Some(&'/') {
            for character in chars.by_ref() {
                if character == '\n' {
                    break;
                }
            }
        } else if character == '/' && chars.peek() == Some(&'*') {
            chars.next();
            while let Some(character) = chars.next() {
                if character == '*' && chars.peek() == Some(&'/') {
                    chars.next();
                    break;
                }
            }
        } else if matches!(character, '\'' | '"' | '`') {
            let quote = character;
            let mut value = String::new();
            while let Some(character) = chars.next() {
                if character == quote {
                    break;
                }
                if character != '\\' {
                    value.push(character);
                    continue;
                }
                let Some(escape) = chars.next() else {
                    break;
                };
                if let Some(decoded) = escaped(escape, &mut chars) {
                    value.push(decoded);
                }
            }
            if quote == '`' && value.contains("${") {
                output.extend(tokens(&value));
            }
            output.push(value);
        } else if character.is_alphanumeric() || matches!(character, '_' | '$' | '\\') {
            let mut value = String::new();
            let mut next = Some(character);
            while let Some(character) = next {
                if character == '\\' {
                    if let Some(escape) = chars.next()
                        && let Some(decoded) = escaped(escape, &mut chars)
                    {
                        value.push(decoded);
                    }
                } else {
                    value.push(character);
                }
                next = if chars.peek().is_some_and(|character| {
                    character.is_alphanumeric() || matches!(character, '_' | '$' | '\\')
                }) {
                    chars.next()
                } else {
                    None
                };
            }
            output.push(value);
        }
    }
    output
}

/// Decode the same JavaScript Unicode syntax in quoted properties and escaped identifiers.
fn escaped(escape: char, chars: &mut std::iter::Peekable<std::str::Chars<'_>>) -> Option<char> {
    if !matches!(escape, 'x' | 'u') {
        return Some(escape);
    }
    let digits = if escape == 'u' && chars.peek() == Some(&'{') {
        chars.next();
        let mut digits = String::new();
        for digit in chars.by_ref() {
            if digit == '}' {
                break;
            }
            digits.push(digit);
        }
        digits
    } else {
        chars.by_ref().take(if escape == 'x' { 2 } else { 4 }).collect()
    };
    u32::from_str_radix(&digits, 16).ok().and_then(char::from_u32)
}
