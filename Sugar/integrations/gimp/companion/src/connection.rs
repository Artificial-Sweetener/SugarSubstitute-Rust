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
//! Validate the prototype's local endpoint and bounded ComfyUI readiness response.

use serde::Serialize;
use std::{
    fmt,
    net::{IpAddr, Ipv4Addr, SocketAddr},
};
use url::{Host, Url};

/// Validated loopback connection configuration; native requests cannot choose another destination.
#[derive(Clone, Debug)]
pub struct ComfyEndpoint {
    url: Url,
    socket: SocketAddr,
}

/// Report invalid configuration without retaining credentials or untrusted native paths.
#[derive(Debug)]
pub struct EndpointError;

impl fmt::Display for EndpointError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(
            "Configure an HTTP loopback ComfyUI address without credentials, query or fragment.",
        )
    }
}
impl std::error::Error for EndpointError {}

impl ComfyEndpoint {
    /// Accept one loopback server without DNS resolution or arbitrary native network access.
    pub fn parse(value: &str) -> Result<Self, EndpointError> {
        let url = Url::parse(value).map_err(|_| EndpointError)?;
        if url.scheme() != "http"
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || url.path() != "/"
        {
            return Err(EndpointError);
        }
        let ip = match url.host() {
            Some(Host::Ipv4(ip)) if ip.is_loopback() => IpAddr::V4(ip),
            Some(Host::Ipv6(ip)) if ip.is_loopback() => IpAddr::V6(ip),
            Some(Host::Domain("localhost")) => IpAddr::V4(Ipv4Addr::LOCALHOST),
            _ => return Err(EndpointError),
        };
        let port = url.port_or_known_default().ok_or(EndpointError)?;
        Ok(Self {
            url,
            socket: SocketAddr::new(ip, port),
        })
    }

    /// Return the canonical origin used for both navigation and native probes.
    pub fn address(&self) -> &str {
        self.url.as_str()
    }

    /// Return the exact loopback socket admitted at configuration time.
    pub fn socket(&self) -> SocketAddr {
        self.socket
    }

    /// Validate a bounded readiness body instead of treating an arbitrary open port as ComfyUI.
    pub fn accepts_readiness(body: &[u8]) -> bool {
        if body.len() > 65536 {
            return false;
        }
        let Ok(value) = serde_json::from_slice::<serde_json::Value>(body) else {
            return false;
        };
        value
            .get("system")
            .is_some_and(serde_json::Value::is_object)
            && value
                .get("devices")
                .is_some_and(serde_json::Value::is_array)
    }
}

/// Return the companion's connection state without exposing Tauri internals to presentation code.
#[derive(Clone, Serialize)]
pub struct ConnectionState {
    /// Describe the completed connection attempt.
    pub phase: &'static str,
    /// Identify the already-validated ComfyUI origin.
    pub address: String,
    /// Explain the next action in the local shell.
    pub message: String,
}
