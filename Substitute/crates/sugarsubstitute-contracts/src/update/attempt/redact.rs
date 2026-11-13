// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Make diagnostic text safe to hand over: local paths, network addresses and anything that
//! looks like a credential are replaced before text reaches an update report.
//!
//! [`sanitize`] is a fixed point: sanitizing its own output changes nothing. The attempt
//! channel relies on that to refuse a report a helper did not sanitize, so a rule added here
//! must keep it (the tests check every example twice).

/// Replacement for a local path, including the words of a path that contains spaces.
pub const PATH: &str = "<path>";
/// Replacement for a URL, host and port, IP address or e-mail address.
pub const ADDRESS: &str = "<address>";
/// Replacement for a credential, a credential's value or a token-shaped word.
pub const REDACTED: &str = "<redacted>";

/// Shortest word treated as a bare token (keys, session identifiers, signed blobs).
const TOKEN_CHARS: usize = 32;

/// Markers whose word carries a credential, either after the marker or in the next word.
const SECRET_MARKERS: [&str; 24] = [
    "token=",
    "token:",
    "password=",
    "password:",
    "passwd=",
    "pwd=",
    "secret=",
    "secret:",
    "key=",
    "apikey:",
    "api-key:",
    "ticket=",
    "signature=",
    "sig=",
    "auth=",
    "authorization:",
    "authorization=",
    "credential=",
    "credentials=",
    "session=",
    "sessionid=",
    "cookie=",
    "cookie:",
    "set-cookie:",
];

/// Authorization schemes whose next word is the credential itself, after a credential marker.
const SCHEMES: [&str; 3] = ["bearer", "basic", "digest"];

/// The one scheme name redacted on its own: "basic" and "digest" are also ordinary words.
const BEARER: &str = "bearer";

/// Prefixes of well-known access tokens that are redacted at any length.
const TOKEN_PREFIXES: [&str; 9] =
    ["ghp_", "gho_", "ghu_", "ghs_", "ghr_", "github_pat_", "glpat-", "xoxb-", "xoxp-"];

/// What the previous word asks of the next one.
#[derive(Clone, Copy, Eq, PartialEq)]
enum Carry {
    /// Nothing; classify the next word on its own.
    None,
    /// An unquoted path ended; a next word with a separator still belongs to it.
    Path,
    /// A quoted path is open until a word holding this quote closes it.
    QuotedPath(char),
    /// A credential's value follows.
    Secret,
}

/// Replace local paths, network addresses and credential-looking words, drop control and
/// direction-override characters, collapse whitespace, and bound the result to
/// [`super::MAX_CAUSE_BYTES`] so a report can be handed over as it is.
pub fn sanitize(text: &str) -> String {
    let cleaned: String = text
        .chars()
        .filter(|&c| c.is_whitespace() || !(c.is_control() || is_direction_override(c)))
        .collect();
    let mut words: Vec<&str> = Vec::new();
    let mut carry = Carry::None;
    for word in cleaned.split_whitespace() {
        let (replacement, next) = classify(word, carry);
        carry = next;
        let Some(replacement) = replacement else { continue };
        let placeholder = [PATH, ADDRESS, REDACTED].contains(&replacement);
        if !(placeholder && words.last() == Some(&replacement)) {
            words.push(replacement);
        }
    }
    bounded(&words.join(" "), super::MAX_CAUSE_BYTES)
}

/// The output for `word` after `carry`, or `None` when it joins the previous placeholder, and
/// what it asks of the next word.
fn classify(word: &str, carry: Carry) -> (Option<&str>, Carry) {
    let bare = word.trim_matches(|c: char| {
        matches!(c, '"' | '\'' | '`' | '(' | ')' | '[' | ']' | '{' | '}' | '<' | '>' | ',' | ';')
    });
    let core = bare.trim_end_matches(['.', ':', '!', '?']);
    match carry {
        Carry::QuotedPath(quote) => {
            let next = if word.contains(quote) { Carry::None } else { Carry::QuotedPath(quote) };
            return (None, next);
        }
        Carry::Path if has_separator(bare) && !bare.contains("://") => {
            return (None, Carry::Path);
        }
        Carry::Secret => {
            let scheme = SCHEMES.contains(&core.to_ascii_lowercase().as_str());
            return (Some(REDACTED), if scheme { Carry::Secret } else { Carry::None });
        }
        Carry::None | Carry::Path => {}
    }
    if is_address(word, bare, core) {
        return (Some(ADDRESS), Carry::None);
    }
    let lower = bare.to_ascii_lowercase();
    if let Some(marker) = SECRET_MARKERS.iter().find(|marker| lower.contains(*marker)) {
        let value_follows = lower.ends_with(marker);
        return (Some(REDACTED), if value_follows { Carry::Secret } else { Carry::None });
    }
    if core.eq_ignore_ascii_case(BEARER) {
        return (Some(word), Carry::Secret);
    }
    if is_path(bare) {
        let quote = word.chars().next().filter(|c| matches!(c, '"' | '\'' | '`'));
        let next = match quote {
            Some(quote) if !word[quote.len_utf8()..].contains(quote) => Carry::QuotedPath(quote),
            _ => Carry::Path,
        };
        return (Some(PATH), next);
    }
    if is_token(core) {
        return (Some(REDACTED), Carry::None);
    }
    (Some(word), Carry::None)
}

/// Drive-qualified, UNC, verbatim, rooted, home-relative, dot-relative and environment-variable
/// paths, any word with a backslash, and words with two or more forward-slash separators.
fn is_path(word: &str) -> bool {
    let bytes = word.as_bytes();
    let slashes = word.matches('/').count();
    word.contains('\\')
        || (word.starts_with('/') && word.len() > 1)
        || ["~/", "./", "../", "$HOME", "${"].iter().any(|prefix| word.starts_with(prefix))
        || (word.starts_with('%') && word[1..].contains('%'))
        || (slashes >= 2 && word.chars().any(|c| c.is_alphabetic()))
        || bytes
            .windows(3)
            .any(|window| window[0].is_ascii_alphabetic() && window[1] == b':' && window[2] == b'/')
}

/// Whether a word can continue a path that contains spaces.
fn has_separator(word: &str) -> bool {
    word.contains(['\\', '/'])
}

/// URLs, e-mail addresses, IPv4 and IPv6 addresses, and `host:port` with a dotted host or
/// `localhost`.
fn is_address(word: &str, bare: &str, core: &str) -> bool {
    if word.contains("://") {
        return true;
    }
    if core.split_once('@').is_some_and(|(user, domain)| !user.is_empty() && is_domain(domain)) {
        return true;
    }
    if bare.matches(':').count() >= 2 && core.chars().all(|c| c.is_ascii_hexdigit() || c == ':') {
        return core.contains("::") || core.matches(':').count() >= 7;
    }
    if word.starts_with('[') && word.contains("]:") {
        return true;
    }
    let (host, port) = core.rsplit_once(':').unwrap_or((core, ""));
    let has_port = !port.is_empty() && port.len() <= 5 && port.bytes().all(|b| b.is_ascii_digit());
    let named = host.eq_ignore_ascii_case("localhost") || is_domain(host);
    is_ipv4(host) || (has_port && named)
}

/// Dot-separated host labels ending in an alphabetic top-level label.
fn is_domain(host: &str) -> bool {
    host.contains('.')
        && host
            .rsplit('.')
            .next()
            .is_some_and(|tld| tld.len() >= 2 && tld.bytes().all(|b| b.is_ascii_alphabetic()))
        && host.split('.').all(|label| {
            !label.is_empty() && label.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        })
}

/// Four dot-separated decimal octets.
fn is_ipv4(host: &str) -> bool {
    let octets: Vec<&str> = host.split('.').collect();
    octets.len() == 4
        && octets
            .iter()
            .all(|octet| !octet.is_empty() && octet.len() <= 3 && octet.parse::<u8>().is_ok())
}

/// Well-known access-token prefixes, signed web tokens, long hexadecimal strings, and long
/// mixed-case words of token characters. Lower-case identifiers such as protocol names and
/// dotted file names stay readable.
fn is_token(word: &str) -> bool {
    let lower = word.to_ascii_lowercase();
    if TOKEN_PREFIXES.iter().any(|prefix| lower.starts_with(prefix) && word.len() > prefix.len()) {
        return true;
    }
    if lower.starts_with("eyj") && word.matches('.').count() == 2 && word.len() >= TOKEN_CHARS {
        return true;
    }
    if word.len() < TOKEN_CHARS {
        return false;
    }
    let bytes = word.as_bytes();
    let hex = bytes.iter().all(u8::is_ascii_hexdigit);
    let mixed = bytes.iter().all(|b| b.is_ascii_alphanumeric() || b"-_+/=".contains(b))
        && bytes.iter().any(u8::is_ascii_digit)
        && bytes.iter().any(u8::is_ascii_uppercase)
        && bytes.iter().any(u8::is_ascii_lowercase);
    hex || mixed
}

/// Bidirectional controls that can make a report read differently from what it contains.
fn is_direction_override(c: char) -> bool {
    matches!(c, '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
}

/// Truncate at a character boundary within `maximum` bytes.
pub(crate) fn bounded(text: &str, maximum: usize) -> String {
    if text.len() <= maximum {
        return text.to_owned();
    }
    let mut end = maximum;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    text[..end].to_owned()
}

#[cfg(test)]
#[path = "redact_tests.rs"]
mod tests;
