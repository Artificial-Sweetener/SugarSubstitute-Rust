// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Decode upstream option data at the provisioning boundary.
use super::*;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
/// Only upstream-owned choice labels and opaque values cross into presentation.
struct OptionWire {
    value: String,
    label: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    recommended: bool,
    selection_token: Option<String>,
}

/// Decode the upstream wire list once; later navigation uses owned typed options.
pub(in crate::setup::controller) fn options(value: Value) -> Result<Vec<Choice>> {
    let options: Vec<OptionWire> = serde_json::from_value(value)?;
    if options.len() > 256 {
        return Err("Installer option admission exceeded".into());
    }
    Ok(options
        .into_iter()
        .map(|option| Choice {
            value: option.value,
            label: option.label,
            description: option.description,
            recommended: option.recommended,
            selection_token: option.selection_token,
        })
        .collect())
}
