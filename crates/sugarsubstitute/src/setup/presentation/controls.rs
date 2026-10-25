// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Stable installer action inventory shared by native construction and typed input routing.
use super::*;
use sugarsubstitute_ui::{availability::Availability, form::Role};
impl Action {
    /// Every original peer in this installer lifetime; hidden pages retain their native identities.
    pub const ALL: [Self; 21] = [
        Self::Close,
        Self::Primary,
        Self::Back,
        Self::Locale,
        Self::Path(PathField::Installation),
        Self::Path(PathField::Comfy),
        Self::Path(PathField::Python),
        Self::Path(PathField::Models),
        Self::Path(PathField::Outputs),
        Self::Browse(PathField::Installation),
        Self::Browse(PathField::Comfy),
        Self::Browse(PathField::Python),
        Self::Browse(PathField::Models),
        Self::Browse(PathField::Outputs),
        Self::Runtime(RuntimeMode::Managed),
        Self::Runtime(RuntimeMode::Attached),
        Self::Option(OptionField::Release),
        Self::Option(OptionField::ComfyVersion),
        Self::Option(OptionField::Variant),
        Self::Mirrors,
        Self::Details,
    ];
    /// Resolve native identity through the authoritative typed action mapping.
    pub fn from_id(id: ControlId) -> Option<Self> {
        Self::ALL.into_iter().find(|action| action.id() == id)
    }
    /// Construct hidden peers before the first accepted page replaces their labels and values.
    pub fn inventory(locale: &str) -> Vec<Control> {
        let label = installer::messages(locale).title;
        Self::ALL
            .into_iter()
            .map(|action| Control {
                id: action.id(),
                label,
                bounds: Bounds { x: 0.0, y: 0.0, width: 200.0, height: 33.0 },
                availability: Availability { visible: false, enabled: false },
                role: match action {
                    Self::Path(_) => Role::TextField(String::new()),
                    Self::Locale | Self::Option(_) => {
                        Role::Select { labels: vec![], selected: None }
                    }
                    Self::Runtime(_) => Role::Radio(false),
                    Self::Mirrors => Role::Check(false),
                    _ => Role::Button,
                },
                text_revision: 0,
            })
            .collect()
    }
}
