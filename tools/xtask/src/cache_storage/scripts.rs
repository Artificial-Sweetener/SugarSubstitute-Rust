// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Restrict script filesystem acquisition and upstream storage routes to exact reviewed boundaries.
//!
//! Conservative tokens include import/require aliases and quoted property names. They do not
//! establish arbitrary computed-call equivalence; native behavior and consumer tests remain required.
use super::*;
use crate::source::fingerprint;
use std::collections::BTreeMap;

pub(super) const EXTENSIONS: &[&str] = &[
    "rs", "ts", "tsx", "mts", "cts", "js", "jsx", "mjs", "cjs", "c", "cc", "cpp", "cxx", "h",
    "hpp", "hxx", "m", "mm", "html", "py", "ps1", "patch",
];

/// Exact source review for an external filesystem concern or the native cache injection boundary.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Boundary {
    pub(crate) path: String,
    pub(crate) owner: String,
    pub(crate) source_sha256: String,
    pub(crate) rationale: String,
    pub(crate) role: Role,
}

/// Reviewed external runtime work, native injection and explicit fixture actors grant different access.
#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Role {
    ExternalRuntime,
    NativeInjection,
    Fixture,
}

/// Reject new raw filesystem acquisition in any authored runtime script, including tests.
pub(super) fn check(
    root: &Path,
    architecture: &Architecture,
    boundaries: &[Boundary],
) -> Result<()> {
    let mut reviewed = BTreeMap::new();
    for boundary in boundaries {
        require(!boundary.rationale.trim().is_empty(), "cache script boundary needs a rationale")?;
        require(
            reviewed.insert(boundary.path.as_str(), boundary).is_none(),
            "duplicate cache script boundary",
        )?;
        let path = resolve(root, &boundary.path)?;
        let owners: Vec<_> = architecture
            .owners
            .iter()
            .filter(|owner| {
                owner.layer != "tooling"
                    && owner.roots().any(|relative| path.starts_with(root.join(relative)))
            })
            .collect();
        require(
            owners.len() == 1 && owners[0].package == boundary.owner,
            "cache script boundary has foreign ownership",
        )?;
        require(
            fingerprint(root, std::slice::from_ref(&boundary.path))? == boundary.source_sha256,
            format!("{}: stale cache script boundary", boundary.path),
        )?;
        let access = access(&read(&path)?);
        let valid = match boundary.role {
            Role::ExternalRuntime => access.builtin && !access.upstream,
            Role::NativeInjection => access.upstream && !access.builtin && access.native,
            Role::Fixture => boundary.path.contains("/tests/") || boundary.path.contains(".test."),
        };
        require(valid, format!("{}: invalid cache script boundary role", boundary.path))?;
    }
    let mut used = BTreeSet::new();
    for owner in architecture.owners.iter().filter(|owner| owner.layer != "tooling") {
        for relative in owner.roots() {
            for path in source_files(&resolve(root, relative)?, EXTENSIONS)? {
                let extension = path
                    .extension()
                    .and_then(|value| value.to_str())
                    .unwrap_or("")
                    .to_ascii_lowercase();
                if !matches!(
                    extension.as_str(),
                    "ts" | "tsx" | "mts" | "cts" | "js" | "jsx" | "mjs" | "cjs" | "html"
                ) {
                    continue;
                }
                let relative = path
                    .strip_prefix(root)
                    .map_err(|cause| Failure::external("resolve script boundary", cause))?
                    .to_string_lossy()
                    .replace('\\', "/");
                if raw_filesystem(&read(&path)?) {
                    require(
                        reviewed.contains_key(relative.as_str()),
                        format!(
                            "{relative}: raw filesystem acquisition requires its exact external-owner review; use the native cache capability for cached downloads"
                        ),
                    )?;
                    used.insert(relative);
                }
            }
        }
    }
    require(
        used.iter().map(String::as_str).collect::<BTreeSet<_>>()
            == reviewed.keys().copied().collect(),
        "remove unused cache script boundaries",
    )
}

/// Inspect known module identities before imported bindings can be renamed or destructured.
fn raw_filesystem(source: &str) -> bool {
    let access = access(source);
    access.builtin || access.upstream
}

/// Finite classifications keep a raw filesystem implementation from posing as native injection.
#[derive(Default)]
struct Access {
    builtin: bool,
    upstream: bool,
    native: bool,
}
fn access(source: &str) -> Access {
    let mut access = Access::default();
    for token in tokens(source) {
        let normalized = token.replace('\\', "/");
        let token = normalized.as_str();
        let module =
            token.strip_suffix(".ts").or_else(|| token.strip_suffix(".js")).unwrap_or(token);
        access.upstream |= module.ends_with("/lib/download") || module.ends_with("/lib/cache");
        access.native |= token == "nativeDownloads";
        access.builtin |= matches!(
            token,
            "fs" | "fs/promises"
                | "node:fs"
                | "node:fs/promises"
                | "original-fs"
                | "fs-extra"
                | "getBuiltinModule"
                | "_linkedBinding"
                | "internalBinding"
        );
    }
    access
}

/// Known cache namespaces cannot be reconstructed directly in another authored language.
pub(super) fn namespace_bypass(source: &str, namespaces: &BTreeSet<String>) -> bool {
    tokens(source).iter().any(|token| {
        token.split(['/', '\\']).any(|part| known_namespace(part.as_bytes(), namespaces))
    })
}

/// Ignore comments, decode quoted escapes and retain identifiers and computed string properties.
fn tokens(source: &str) -> Vec<String> {
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

#[cfg(test)]
mod tests;
