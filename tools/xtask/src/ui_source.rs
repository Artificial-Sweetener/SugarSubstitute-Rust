// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Recognize UI construction capabilities in source, including inactive code and macros.

use std::collections::{BTreeMap, BTreeSet};

use proc_macro2::{TokenStream, TokenTree};
use serde::Deserialize;
use syn::visit::Visit;

use crate::{
    error::{Failure, Result},
    workspace::Dependency,
};

/// A registered boundary identifies the kind of presentation authority it consumes.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Capability {
    Raster,
    Text,
    Window,
    Webview,
}

/// Canonical Cargo identities prevent dependency renaming from bypassing ownership.
pub(crate) fn dependency(name: &str) -> Option<Capability> {
    match name.replace('-', "_").as_str() {
        "tiny_skia" | "resvg" | "usvg" | "skia_safe" | "softbuffer" | "cairo_rs" => {
            Some(Capability::Raster)
        }
        "cosmic_text" => Some(Capability::Text),
        "tao"
        | "winit"
        | "gtk"
        | "gtk4"
        | "egui"
        | "eframe"
        | "iced"
        | "slint"
        | "gpui"
        | "druid"
        | "fltk"
        | "native_windows_gui"
        | "native_windows_derive" => Some(Capability::Window),
        "wry" | "webview2_com" | "web_view" | "tauri" => Some(Capability::Webview),
        _ => None,
    }
}

/// Inspect syntax, never prose or ordinary string literals.
pub(crate) fn findings(source: &str, dependencies: &[Dependency]) -> Result<BTreeSet<Capability>> {
    let tree =
        syn::parse_file(source).map_err(|e| Failure::external("parse UI boundary source", e))?;
    let mut scanner = Scanner {
        aliases: dependencies
            .iter()
            .filter_map(|d| {
                dependency(&d.name)
                    .map(|kind| (d.rename.as_ref().unwrap_or(&d.name).replace('-', "_"), kind))
            })
            .collect(),
        found: BTreeSet::new(),
    };
    scanner
        .tokens(source.parse().map_err(|e| Failure::external("tokenize UI boundary source", e))?);
    scanner.visit_file(&tree);
    Ok(scanner.found)
}

struct Scanner {
    aliases: BTreeMap<String, Capability>,
    found: BTreeSet<Capability>,
}

impl Scanner {
    /// Macro arguments and inactive target/test branches remain governed code.
    fn tokens(&mut self, stream: TokenStream) {
        let tokens: Vec<_> = stream.into_iter().collect();
        if let [TokenTree::Ident(name), TokenTree::Punct(equal), TokenTree::Literal(value)] =
            tokens.as_slice()
            && name.to_string().trim_start_matches("r#") == "link_name"
            && equal.as_char() == '='
            && let Ok(name) = syn::parse_str::<syn::LitStr>(&value.to_string())
        {
            self.identifier(&name.value());
        }
        for token in tokens {
            match token {
                TokenTree::Group(group) => self.tokens(group.stream()),
                TokenTree::Ident(name) => {
                    self.identifier(name.to_string().trim_start_matches("r#"))
                }
                _ => {}
            }
        }
    }

    /// Imports retain the original symbol even when the caller renames it locally.
    fn identifier(&mut self, name: &str) {
        let kind = match name {
            "tiny_skia"
            | "resvg"
            | "usvg"
            | "skia_safe"
            | "softbuffer"
            | "cairo"
            | "fill_rect"
            | "fill_path"
            | "stroke_path"
            | "BitBlt"
            | "StretchDIBits"
            | "CreateDIBSection"
            | "CreateSolidBrush"
            | "SetTextColor"
            | "SetBkColor"
            | "SetDCBrushColor"
            | "DrawTextW"
            | "DrawTextExW"
            | "RoundRect"
            | "FillRect"
            | "GradientFill"
            | "CoverageRaster"
            | "Bitmap"
            | "BitmapView"
            | "CoverageMask"
            | "PaintCursor"
            | "append_cubic"
            | "byte_product"
            | "blend_pixel"
            | "rounded"
            | "FillRoundedRectangle"
            | "FillRectangle"
            | "DrawRoundedRectangle"
            | "DrawRectangle"
            | "FillGeometry"
            | "DrawGeometry"
            | "DrawText"
            | "DrawTextLayout"
            | "CreateSolidColorBrush"
            | "D2D1CreateFactory"
            | "CreateBitmapFromWicBitmap"
            | "CreateDCRenderTarget"
            | "CreateHwndRenderTarget" => Some(Capability::Raster),
            "cosmic_text"
            | "CreateTextFormat"
            | "CreateTextLayout"
            | "CreateGdiCompatibleTextLayout"
            | "IDWriteTextRenderer"
            | "CreateFontW"
            | "CreateFontIndirectW" => Some(Capability::Text),
            "WindowBuilder"
            | "CreateWindowExW"
            | "CreateWindowExA"
            | "CreateWindowW"
            | "CreateWindowA"
            | "CreateDialogParamW"
            | "CreateDialogIndirectParamW"
            | "DialogBoxParamW"
            | "DialogBoxIndirectParamW"
            | "MessageBoxW"
            | "MessageBoxA"
            | "TaskDialog"
            | "TaskDialogIndirect"
            | "RegisterClassExW"
            | "RegisterClassW" => Some(Capability::Window),
            "WebViewBuilder" | "CreateCoreWebView2EnvironmentWithOptions" => {
                Some(Capability::Webview)
            }
            _ => self.aliases.get(name).copied().filter(|_| !matches!(name, "tao" | "winit")),
        };
        if let Some(kind) = kind {
            self.found.insert(kind);
        }
    }
}

impl<'ast> Visit<'ast> for Scanner {
    /// Renamed foreign functions retain their actual linked native identity.
    fn visit_attribute(&mut self, attribute: &'ast syn::Attribute) {
        if let syn::Meta::NameValue(value) = &attribute.meta
            && value.path.is_ident("link_name")
            && let syn::Expr::Lit(literal) = &value.value
            && let syn::Lit::Str(name) = &literal.lit
        {
            self.identifier(&name.value());
        }
        syn::visit::visit_attribute(self, attribute);
    }
}
