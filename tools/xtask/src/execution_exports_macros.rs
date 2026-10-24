// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Inspect public declarative expansions as paths rather than arbitrary identifier bags.

use crate::execution_exports_names::Names;
use proc_macro2::{Delimiter, TokenStream, TokenTree};
use std::collections::BTreeSet;

/// Collect diagnostics and local helper signatures exposed by emitted calls.
#[derive(Default)]
pub(crate) struct Findings {
    pub(crate) messages: BTreeSet<String>,
    pub(crate) helpers: BTreeSet<Vec<String>>,
}

/// Check exported definitions and re-exports at their actual lexical scopes.
pub(crate) fn findings(
    names: &Names,
    module: &[String],
    items: &[syn::Item],
    restricted: fn(&[String]) -> bool,
) -> Findings {
    let mut scanner = Scanner::new(names, restricted);
    for item in items {
        match item {
            syn::Item::Macro(item) if exported(&item.attrs) => {
                scanner.definition(module, &item.mac.tokens)
            }
            syn::Item::Use(item) if matches!(item.vis, syn::Visibility::Public(_)) => {
                let prefix =
                    if item.leading_colon.is_some() { vec![String::new()] } else { Vec::new() };
                for path in use_paths(&item.tree, prefix) {
                    if let Ok(path) = syn::parse_str::<syn::Path>(&path.join("::")) {
                        for resolved in names.resolve(module, &path, &BTreeSet::new()) {
                            scanner.local_definition(&resolved);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    scanner.found
}

/// Inspect initializer arguments and statically resolved local declarative expansions.
pub(crate) fn invocation(
    names: &Names,
    module: &[String],
    item: &syn::Macro,
    restricted: fn(&[String]) -> bool,
) -> Findings {
    let mut scanner = Scanner::new(names, restricted);
    for resolved in names.resolve(module, &item.path, &BTreeSet::new()) {
        scanner.path(&resolved);
        scanner.local_definition(&resolved);
    }
    scanner.transcriber(module, item.tokens.clone());
    scanner.found
}

/// Recognize direct and conditionally exported macros without evaluating the condition.
pub(crate) fn exported(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        crate::execution_exports_modules::attribute_contains(&attr.meta, "macro_export")
    })
}

/// Guard recursive expansions while retaining scheduler paths and callable identities.
struct Scanner<'a> {
    names: &'a Names,
    restricted: fn(&[String]) -> bool,
    found: Findings,
    visiting: BTreeSet<Vec<String>>,
}

impl<'a> Scanner<'a> {
    /// Start one audit without cross-expansion scope leakage.
    fn new(names: &'a Names, restricted: fn(&[String]) -> bool) -> Self {
        Self { names, restricted, found: Findings::default(), visiting: BTreeSet::new() }
    }

    /// Expand every local platform alternative once on the current recursion path.
    fn local_definition(&mut self, path: &[String]) {
        if !self.visiting.insert(path.to_vec()) {
            return;
        }
        for (module, item) in self.names.macro_items(path) {
            self.definition(module, &item.mac.tokens);
        }
        self.visiting.remove(path);
    }

    /// Matcher tokens grant no capability; inspect transcribers following rule arrows.
    fn definition(&mut self, module: &[String], stream: &TokenStream) {
        let tokens: Vec<_> = stream.clone().into_iter().collect();
        for window in tokens.windows(3) {
            if punctuation(&window[0], '=')
                && punctuation(&window[1], '>')
                && let TokenTree::Group(group) = &window[2]
            {
                self.transcriber(module, group.stream());
            }
        }
    }

    /// Follow literal paths, local calls and explicit metavariable path composition.
    fn transcriber(&mut self, module: &[String], stream: TokenStream) {
        let tokens: Vec<_> = stream.into_iter().collect();
        for (index, token) in tokens.iter().enumerate() {
            if let TokenTree::Group(group) = token {
                self.transcriber(module, group.stream());
            }
            if punctuation(token, '$')
                && matches!(tokens.get(index + 1), Some(TokenTree::Ident(name)) if name != "crate")
                && ((tokens.get(index + 2).is_some_and(|token| punctuation(token, ':'))
                    && tokens.get(index + 3).is_some_and(|token| punctuation(token, ':')))
                    || (index >= 2
                        && punctuation(&tokens[index - 1], ':')
                        && punctuation(&tokens[index - 2], ':')))
            {
                self.found.messages.insert("exported macro assembles a caller-selected path; expose an owned operation explicitly".into());
            }
            let TokenTree::Ident(name) = token else { continue };
            let mut parts = vec![name.to_string()];
            let mut end = index + 1;
            while end + 2 < tokens.len()
                && punctuation(&tokens[end], ':')
                && punctuation(&tokens[end + 1], ':')
            {
                let TokenTree::Ident(name) = &tokens[end + 2] else { break };
                parts.push(name.to_string());
                end += 3;
            }
            let imported = index > 0
                && matches!(&tokens[index - 1], TokenTree::Ident(keyword) if keyword == "use" || keyword == "crate");
            let callable = matches!(tokens.get(end), Some(TokenTree::Group(group)) if group.delimiter() == Delimiter::Parenthesis);
            let expansion = tokens.get(end).is_some_and(|token| punctuation(token, '!'));
            if parts.len() == 1 && !imported && !callable && !expansion {
                continue;
            }
            let absolute = index >= 2
                && punctuation(&tokens[index - 1], ':')
                && punctuation(&tokens[index - 2], ':')
                && (index == 2 || !matches!(&tokens[index - 3], TokenTree::Ident(_)));
            let spelling = format!("{}{}", if absolute { "::" } else { "" }, parts.join("::"));
            if let Ok(path) = syn::parse_str::<syn::Path>(&spelling) {
                for resolved in self.names.resolve(module, &path, &BTreeSet::new()) {
                    self.path(&resolved);
                    if !self.names.functions(&resolved).is_empty() {
                        self.found.helpers.insert(resolved.clone());
                    }
                    if expansion {
                        self.local_definition(&resolved);
                    }
                }
            }
        }
    }

    /// Classify canonical paths with the same rule as ordinary source access.
    fn path(&mut self, resolved: &[String]) {
        if (self.restricted)(resolved) {
            self.found.messages.insert(format!(
                "exported macro exposes restricted capability {}",
                resolved.join("::")
            ));
        }
    }
}

/// Flatten import leaves while preserving aliases and module self imports.
fn use_paths(tree: &syn::UseTree, mut prefix: Vec<String>) -> Vec<Vec<String>> {
    match tree {
        syn::UseTree::Path(path) => {
            prefix.push(path.ident.to_string());
            use_paths(&path.tree, prefix)
        }
        syn::UseTree::Group(group) => {
            group.items.iter().flat_map(|item| use_paths(item, prefix.clone())).collect()
        }
        syn::UseTree::Name(name) => {
            if name.ident != "self" {
                prefix.push(name.ident.to_string());
            }
            vec![prefix]
        }
        syn::UseTree::Rename(name) => {
            if name.ident != "self" {
                prefix.push(name.ident.to_string());
            }
            vec![prefix]
        }
        syn::UseTree::Glob(_) => Vec::new(),
    }
}

/// Compare punctuation without matching literals or field identifiers.
fn punctuation(token: &TokenTree, value: char) -> bool {
    matches!(token, TokenTree::Punct(token) if token.as_char() == value)
}
