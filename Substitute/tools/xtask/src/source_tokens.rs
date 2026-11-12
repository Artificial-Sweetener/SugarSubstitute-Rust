// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Tokenize conservative authored script/native identities and decoded quoted properties.
/// Decoded identifiers and literals with punctuation retained for context-sensitive property checks.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Token {
    /// An ordinary or escaped source identifier.
    Name(String),
    /// Decoded quoted source text.
    Literal(String),
    /// Non-whitespace syntax outside comments and strings.
    Symbol(char),
}
impl Token {
    /// Borrow decoded text without treating punctuation as an identifier.
    pub(crate) fn text(&self) -> Option<&str> {
        match self {
            Self::Name(value) | Self::Literal(value) => Some(value),
            Self::Symbol(_) => None,
        }
    }
}
/// Ignore comments and expose the decoded name inventory used by conservative capability gates.
pub(crate) fn tokens(source: &str) -> Vec<String> {
    lex(source)
        .into_iter()
        .filter_map(|token| match token {
            Token::Name(value) | Token::Literal(value) => Some(value),
            Token::Symbol(_) => None,
        })
        .collect()
}
/// Decode quoted and identifier escapes while preserving member and concatenation syntax.
pub(crate) fn lex(source: &str) -> Vec<Token> {
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
                output.extend(lex(&value));
            }
            output.push(Token::Literal(value));
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
            output.push(Token::Name(value));
        } else if !character.is_whitespace() {
            output.push(Token::Symbol(character));
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn syntax_distinguishes_literal_concatenation_from_ordinary_calls() {
        assert_eq!(
            lex("object['ap' + 'pend']"),
            vec![
                Token::Name("object".into()),
                Token::Symbol('['),
                Token::Literal("ap".into()),
                Token::Symbol('+'),
                Token::Literal("pend".into()),
                Token::Symbol(']')
            ]
        );
        assert_eq!(tokens("value.get('text') // ignored append"), vec!["value", "get", "text"]);
    }
    #[test]
    fn decoded_member_names_keep_their_original_selection_syntax() {
        assert_eq!(
            lex(r"element.ap\u0070end(text)"),
            vec![
                Token::Name("element".into()),
                Token::Symbol('.'),
                Token::Name("append".into()),
                Token::Symbol('('),
                Token::Name("text".into()),
                Token::Symbol(')')
            ]
        );
    }
}
