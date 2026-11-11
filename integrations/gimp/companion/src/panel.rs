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
//! Validate the public panel's native transport scope without implementing editor operations.

use crate::connection::ComfyEndpoint;
use serde_json::Value;
use std::io::{self, Write};
use url::Url;

/// Identify the compiled adapter rather than allowing remote pages to select a native host.
pub const HOST: &str = "ai.artificialsweetener.sugarcubes.gimp";

/// Bind public envelopes and top-level navigation to the local shell's current session.
#[derive(Clone)]
pub struct PanelRoute {
    url: Url,
    session: String,
}

impl PanelRoute {
    /// Accept only a complete opaque session and the already validated ComfyUI endpoint.
    pub fn new(endpoint: &ComfyEndpoint, session: String) -> Result<Self, String> {
        if !(16..=80).contains(&session.len())
            || !session
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        {
            return Err("Invalid public panel session.".into());
        }
        let mut url = Url::parse(endpoint.address()).map_err(|error| error.to_string())?;
        url.query_pairs_mut()
            .append_pair("sc-panel-host", HOST)
            .append_pair("sc-panel-session", &session)
            .append_pair("sc-panel-transport", "native");
        Ok(Self { url, session })
    }

    /// Supply one fully identified navigation to the guest WebView.
    pub fn url(&self) -> &Url {
        &self.url
    }

    /// Compare the caller's scope without accepting a former navigation's delayed requests.
    pub fn session(&self) -> &str {
        &self.session
    }

    /// Refuse external origins, missing identities and duplicate identity parameters.
    pub fn accepts_navigation(&self, url: &Url) -> bool {
        if !self.accepts_origin(url) {
            return false;
        }
        let pairs: Vec<_> = url.query_pairs().collect();
        [
            ("sc-panel-host", HOST),
            ("sc-panel-session", &self.session),
            ("sc-panel-transport", "native"),
        ]
        .iter()
        .all(|(key, expected)| {
            let mut values = pairs.iter().filter(|(name, _)| name == key);
            values.next().is_some_and(|(_, value)| value == expected) && values.next().is_none()
        })
    }

    /// Retain an authenticated document's session while Comfy's SPA changes its URL in place.
    pub fn accepts_origin(&self, url: &Url) -> bool {
        url.origin() == self.url.origin() && url.username().is_empty() && url.password().is_none()
    }

    /// Admit only public protocol envelopes; shared schema dispatchers validate their operation bodies.
    pub fn accepts_message(&self, message: &Value, maximum: usize) -> bool {
        if message.get("session").and_then(Value::as_str) != Some(&self.session)
            || !matches!(
                message.get("protocol").and_then(Value::as_str),
                Some(
                    "sugarcubes.panel"
                        | "sugarcubes.host"
                        | "sugarcubes.canvas"
                        | "sugarcubes.canvas.masks"
                )
            )
        {
            return false;
        }
        serde_json::to_writer(Budget(maximum), message).is_ok()
    }
}

/// Count serialized bytes without allocating another full raster envelope.
struct Budget(usize);
impl Write for Budget {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.0 {
            return Err(io::Error::other("Public panel byte budget exceeded."));
        }
        self.0 -= bytes.len();
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
