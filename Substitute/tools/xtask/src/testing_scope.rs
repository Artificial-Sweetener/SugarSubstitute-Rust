// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors

//! Find the Rust source that runs only as tests, following each crate's actual module tree.
//!
//! Test scope is a crate's `tests/` directory, every item or module whose `cfg` admits only test
//! builds, and every `*_fixture` module, which hosts a native qualification campaign. Modules are
//! resolved through `mod` declarations and `#[path]` exactly as the compiler loads them, so a test
//! module cannot leave scope by its file name. A declared module whose file is missing fails.

use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

use syn::spanned::Spanned;

use crate::{
    error::{Failure, Result, read},
    source::rust_files,
};

/// The test-only source of one file: the whole file, or the test-only items of production code.
pub(crate) struct Scoped {
    pub(crate) path: PathBuf,
    pub(crate) source: String,
}

/// Collect every file's test-only source under one crate root.
///
/// # Errors
/// Fails when a file cannot be read or parsed, or a declared module file does not exist.
pub(crate) fn scoped_sources(crate_root: &Path) -> Result<Vec<Scoped>> {
    let mut walker = Walker::default();
    let tests = crate_root.join("tests");
    if tests.is_dir() {
        for path in rust_files(&tests)? {
            let source = read(&path)?;
            walker.scoped.push(Scoped { path, source });
        }
    }
    let src = crate_root.join("src");
    for entry in ["lib.rs", "main.rs"].map(|name| src.join(name)) {
        if entry.is_file() {
            walker.file(&entry, src.clone(), false)?;
        }
    }
    let bin = src.join("bin");
    if bin.is_dir() {
        for path in rust_files(&bin)? {
            let direct = path.parent() == Some(bin.as_path());
            let nested_main = path.file_name().is_some_and(|name| name == "main.rs")
                && path.parent().and_then(Path::parent) == Some(bin.as_path());
            if direct || nested_main {
                let children = if direct { bin.join(stem(&path)) } else { parent(&path) };
                walker.file(&path, children, false)?;
            }
        }
    }
    Ok(walker.scoped)
}

#[derive(Default)]
struct Walker {
    scoped: Vec<Scoped>,
    visited: BTreeSet<PathBuf>,
}

impl Walker {
    /// Load one module file; `children` is the directory its out-of-line child modules live in.
    fn file(&mut self, path: &Path, children: PathBuf, test: bool) -> Result<()> {
        if !self.visited.insert(path.to_path_buf()) {
            return Ok(());
        }
        let text = read(path)?;
        let source = text.strip_prefix('\u{feff}').unwrap_or(&text).to_owned();
        let parsed = syn::parse_file(&source).map_err(|error| {
            Failure::external(format!("parse test scope of {}", path.display()), error)
        })?;
        let mut slices = Vec::new();
        let location = Location { file_dir: parent(path), module_dir: children, top: true };
        self.items(&parsed.items, &source, &location, test, &mut slices)?;
        if test {
            self.scoped.push(Scoped { path: path.to_path_buf(), source });
        } else if !slices.is_empty() {
            self.scoped.push(Scoped { path: path.to_path_buf(), source: slices.join("\n") });
        }
        Ok(())
    }

    fn items(
        &mut self,
        items: &[syn::Item],
        source: &str,
        location: &Location,
        test: bool,
        slices: &mut Vec<String>,
    ) -> Result<()> {
        for item in items {
            let item_test = test || attributes(item).iter().any(test_only);
            match item {
                syn::Item::Mod(module) => {
                    let scoped = item_test || module.ident.to_string().ends_with("_fixture");
                    self.module(module, source, location, scoped, test, slices)?;
                }
                syn::Item::Impl(implementation) if !item_test => {
                    for member in &implementation.items {
                        if let syn::ImplItem::Fn(function) = member
                            && function.attrs.iter().any(test_only)
                        {
                            slices.push(format!("impl Scoped {{ {} }}", slice(source, function)?));
                        }
                    }
                }
                _ if item_test && !test => slices.push(slice(source, item)?.to_owned()),
                _ => {}
            }
        }
        Ok(())
    }

    fn module(
        &mut self,
        module: &syn::ItemMod,
        source: &str,
        location: &Location,
        scoped: bool,
        test: bool,
        slices: &mut Vec<String>,
    ) -> Result<()> {
        let name = module.ident.to_string();
        let path = path_attribute(&module.attrs);
        if let Some((_, content)) = &module.content {
            if scoped && !test {
                slices.push(slice(source, module)?.to_owned());
            }
            let nested = Location {
                file_dir: location.file_dir.clone(),
                module_dir: location.module_dir.join(path.unwrap_or(name)),
                top: false,
            };
            return self.items(content, source, &nested, scoped, slices);
        }
        let (file, children) = if let Some(path) = path {
            let base = if location.top { &location.file_dir } else { &location.module_dir };
            let file = base.join(path);
            let children = parent(&file);
            (file, children)
        } else {
            let flat = location.module_dir.join(format!("{name}.rs"));
            let nested = location.module_dir.join(&name).join("mod.rs");
            let file = if flat.is_file() { flat } else { nested };
            (file, location.module_dir.join(&name))
        };
        if !file.is_file() {
            return Err(Failure::Policy(format!(
                "testing: module {name} has no source file at {}",
                file.display()
            )));
        }
        self.file(&file, children, scoped)
    }
}

/// Where a module's out-of-line children and `#[path]` files resolve from.
struct Location {
    file_dir: PathBuf,
    module_dir: PathBuf,
    top: bool,
}

/// Whether an attribute is a `cfg` that admits only test builds.
fn test_only(attribute: &syn::Attribute) -> bool {
    attribute.path().is_ident("cfg")
        && attribute.parse_args::<syn::Meta>().is_ok_and(|meta| requires_test(&meta))
}

/// `test` itself, an `all` with a test requirement, or an `any` whose every branch requires test.
fn requires_test(meta: &syn::Meta) -> bool {
    match meta {
        syn::Meta::Path(path) => path.is_ident("test"),
        syn::Meta::List(list) => {
            let Ok(arguments) = list.parse_args_with(
                syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated,
            ) else {
                return false;
            };
            if list.path.is_ident("all") {
                arguments.iter().any(requires_test)
            } else if list.path.is_ident("any") {
                !arguments.is_empty() && arguments.iter().all(requires_test)
            } else {
                false
            }
        }
        syn::Meta::NameValue(_) => false,
    }
}

fn path_attribute(attributes: &[syn::Attribute]) -> Option<String> {
    attributes.iter().find_map(|attribute| match &attribute.meta {
        syn::Meta::NameValue(value) if value.path.is_ident("path") => match &value.value {
            syn::Expr::Lit(syn::ExprLit { lit: syn::Lit::Str(path), .. }) => Some(path.value()),
            _ => None,
        },
        _ => None,
    })
}

fn attributes(item: &syn::Item) -> &[syn::Attribute] {
    match item {
        syn::Item::Const(item) => &item.attrs,
        syn::Item::Enum(item) => &item.attrs,
        syn::Item::ExternCrate(item) => &item.attrs,
        syn::Item::Fn(item) => &item.attrs,
        syn::Item::ForeignMod(item) => &item.attrs,
        syn::Item::Impl(item) => &item.attrs,
        syn::Item::Macro(item) => &item.attrs,
        syn::Item::Mod(item) => &item.attrs,
        syn::Item::Static(item) => &item.attrs,
        syn::Item::Struct(item) => &item.attrs,
        syn::Item::Trait(item) => &item.attrs,
        syn::Item::TraitAlias(item) => &item.attrs,
        syn::Item::Type(item) => &item.attrs,
        syn::Item::Union(item) => &item.attrs,
        syn::Item::Use(item) => &item.attrs,
        _ => &[],
    }
}

/// The exact source text of one parsed node, including its attributes.
fn slice<'a>(source: &'a str, node: &impl Spanned) -> Result<&'a str> {
    source
        .get(node.span().byte_range())
        .ok_or_else(|| Failure::Policy("testing: test item span lies outside its source".into()))
}

fn parent(path: &Path) -> PathBuf {
    path.parent().map(Path::to_path_buf).unwrap_or_default()
}

fn stem(path: &Path) -> String {
    path.file_stem().map(|stem| stem.to_string_lossy().into_owned()).unwrap_or_default()
}
