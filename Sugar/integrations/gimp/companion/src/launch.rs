//    SugarCubes - composable workflow units for ComfyUI
//    Copyright (C) 2026  Artificial Sweetener and contributors
//
//    This program is free software: you can redistribute it and/or modify
//    it under the terms of the GNU Affero General Public License as published by
//    the Free Software Foundation, either version 3 of the License, or
//    (at your option) any later version.
//
//    This program is distributed in the hope that it will be useful,
//    but WITHOUT ANY WARRANTY; without even the implied warranty of
//    MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//    GNU Affero General Public License for more details.
//
//    You should have received a copy of the GNU Affero General Public License
//    along with this program.  If not, see <https://www.gnu.org/licenses/>.
//! Admit the plug-in's private process-launch context before constructing native resources.
use std::num::NonZeroUsize;

/// Retain one validated parent association and native pipe admission for a companion lifetime.
pub struct NativeLaunch {
    native: bool,
    owner: Option<NonZeroUsize>,
    title: String,
}

impl NativeLaunch {
    /// Validate native admission and the SDK-supplied owner without consulting global foreground state.
    pub fn parse(
        native: Option<&str>,
        owner: Option<&str>,
        name: Option<&str>,
    ) -> Result<Self, String> {
        let native = match native {
            None => false,
            Some("1") => true,
            _ => return Err("Invalid GIMP native parent configuration.".into()),
        };
        let owner = match owner {
            None => None,
            Some(value)
                if native
                    && !value.is_empty()
                    && value.bytes().all(|byte| byte.is_ascii_digit()) =>
            {
                Some(
                    value
                        .parse::<NonZeroUsize>()
                        .map_err(|_| "Invalid GIMP display window handle.")?,
                )
            }
            Some(_) => return Err("Invalid GIMP display window handle.".into()),
        };
        let name: String = name
            .unwrap_or_default()
            .chars()
            .filter(|value| !value.is_control())
            .take(256)
            .collect();
        let title = if native && !name.is_empty() {
            format!("SugarCubes — {name} — GIMP")
        } else {
            "SugarCubes — GIMP".into()
        };
        Ok(Self {
            native,
            owner,
            title,
        })
    }

    /// Admit stdio only for a process launched by the GIMP plug-in.
    pub fn native(&self) -> bool {
        self.native
    }

    /// Return the explicit invocation window when the native SDK supplies a GUI display.
    pub fn owner(&self) -> Option<NonZeroUsize> {
        self.owner
    }

    /// Identify the launch image without accepting control characters into the OS caption.
    pub fn title(&self) -> &str {
        &self.title
    }
}
