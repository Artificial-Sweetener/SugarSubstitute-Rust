// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Validate one same-origin opaque media route without obtaining filesystem authority.
use super::*;
use crate::{SessionBootstrap, main_navigation::admits_url, main_session_contract::origin_text};
use sugarsubstitute_contracts::desktop_session::{OutputId, WorkspaceEntryId};
pub(crate) const RESERVED_PATH: &str = "/__sugarsubstitute/session-media/v1/";
pub(crate) const MAX_URI_BYTES: usize = 4096;
#[derive(Clone, Copy)]
pub(crate) enum Header<'a> {
    Absent,
    Text(&'a str),
    Unreadable,
}
pub(crate) struct Evidence<'a> {
    pub(crate) uri: &'a str,
    pub(crate) source: &'a str,
    pub(crate) method: &'a str,
    pub(crate) resource: bool,
    pub(crate) origin: Header<'a>,
    pub(crate) referer: Header<'a>,
    pub(crate) site: Header<'a>,
    pub(crate) range: Header<'a>,
}
pub(crate) struct Requested {
    pub(crate) output: String,
    pub(crate) binding: SessionBinding,
    pub(crate) range: Option<SessionMediaRange>,
    pub(crate) head: bool,
}
/// Opaque per-webview route. The cap limits routing and is revoked with its owner; it is not
/// permission to read an asset absent the application's current/pending output binding map.
pub(crate) struct Endpoint {
    bootstrap: SessionBootstrap,
    path: String,
    prefix: String,
    family: String,
    filter: String,
}
impl Endpoint {
    pub(crate) fn new(bootstrap: SessionBootstrap, token: [u8; 16]) -> Self {
        let token = token.iter().map(|byte| format!("{byte:02x}")).collect::<String>();
        let origin = origin_text(bootstrap.origin());
        let path = format!("{RESERVED_PATH}{token}/");
        Self {
            prefix: format!("{origin}{path}"),
            family: format!("{origin}{RESERVED_PATH}"),
            filter: format!("{origin}{RESERVED_PATH}*"),
            bootstrap,
            path,
        }
    }
    pub(crate) fn path(&self) -> &str {
        &self.path
    }
    pub(crate) fn filter(&self) -> &str {
        &self.filter
    }
    pub(crate) fn matches_family(&self, uri: &str) -> bool {
        uri.starts_with(&self.family)
    }
    pub(crate) fn admit(&self, evidence: Evidence<'_>) -> Option<Requested> {
        if !evidence.resource
            || !matches!(evidence.method, "GET" | "HEAD")
            || evidence.uri.len() > MAX_URI_BYTES
            || evidence.uri.contains('#')
            || !admits_url(self.bootstrap.origin(), evidence.source)
        {
            return None;
        }
        let origin = origin_text(self.bootstrap.origin());
        match evidence.origin {
            Header::Text(value) if value == origin => {}
            Header::Absent
                if matches!(evidence.site, Header::Text("same-origin"))
                    || (matches!(evidence.site, Header::Absent)
                        && matches!(evidence.referer, Header::Text(value) if admits_url(self.bootstrap.origin(),value))) =>
            {
                match evidence.referer {
                    Header::Absent => {}
                    Header::Text(value) if admits_url(self.bootstrap.origin(), value) => {}
                    _ => return None,
                }
            }
            _ => return None,
        }
        let route = evidence.uri.strip_prefix(&self.prefix)?;
        let (output, query) = route.split_once('?')?;
        OutputId::new(output).ok()?;
        if !valid_percent(query) {
            return None;
        }
        let mut values = url::form_urlencoded::parse(query.as_bytes());
        let (key, value) = values.next()?;
        if key != "binding" || values.next().is_some() {
            return None;
        }
        let binding: SessionBinding = serde_json::from_str(&value).ok()?;
        if !binding.valid()
            || binding.runtime_session.is_empty()
            || WorkspaceEntryId::new(binding.entry_id.clone()).is_err()
        {
            return None;
        }
        let range = match evidence.range {
            Header::Absent => None,
            Header::Text(value) => Some(SessionMediaRange::parse(value).ok()?),
            Header::Unreadable => return None,
        };
        Some(Requested {
            output: output.to_owned(),
            binding,
            range,
            head: evidence.method == "HEAD",
        })
    }
}
fn valid_percent(value: &str) -> bool {
    let mut decoded = Vec::new();
    if decoded.try_reserve_exact(value.len()).is_err() {
        return false;
    }
    let mut bytes = value.bytes();
    while let Some(byte) = bytes.next() {
        if byte == b'%' {
            let Some(high) = bytes.next().and_then(hex) else { return false };
            let Some(low) = bytes.next().and_then(hex) else { return false };
            decoded.push(high * 16 + low);
        } else {
            decoded.push(byte);
        }
    }
    std::str::from_utf8(&decoded).is_ok()
}
fn hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
    fn endpoint() -> Result<Endpoint> {
        let vectors: serde_json::Value = serde_json::from_str(include_str!(
            "../../../../third_party/sugarcubes-session/session_vectors.json"
        ))?;
        let scope = serde_json::from_value(vectors["scope"].clone())?;
        Ok(Endpoint::new(
            SessionBootstrap::new(
                scope,
                sugarsubstitute_contracts::configuration::LoopbackOrigin::parse(
                    "http://127.0.0.1:8188",
                    8188,
                )?,
                "00000000-0000-4000-8000-000000000001".into(),
            )?,
            [7; 16],
        ))
    }
    fn uri(endpoint: &Endpoint) -> Result<String> {
        let binding = SessionBinding {
            entry_id: "00000000-0000-4000-8000-000000000002".into(),
            runtime_session: "fresh runtime".into(),
            generation: 1,
            graph_revision: 2,
            source_revision: 3,
            navigation_revision: 4,
        };
        let mut query = url::form_urlencoded::Serializer::new(String::new());
        query.append_pair("binding", &serde_json::to_string(&binding)?);
        let query = query.finish();
        Ok(format!("{}00000000-0000-4000-8000-000000000003?{query}", endpoint.prefix))
    }
    fn evidence<'a>(uri: &'a str) -> Evidence<'a> {
        Evidence {
            uri,
            source: "http://127.0.0.1:8188/",
            method: "GET",
            resource: true,
            origin: Header::Absent,
            referer: Header::Absent,
            site: Header::Text("same-origin"),
            range: Header::Absent,
        }
    }
    #[test]
    fn same_origin_image_without_origin_or_referer_uses_actual_webview_and_fetch_source() -> Result
    {
        let endpoint = endpoint()?;
        let uri = uri(&endpoint)?;
        assert!(endpoint.admit(evidence(&uri)).is_some());
        let mut value = evidence(&uri);
        value.source = "http://evil.test/";
        assert!(endpoint.admit(value).is_none());
        let mut value = evidence(&uri);
        value.site = Header::Text("cross-site");
        assert!(endpoint.admit(value).is_none());
        let mut value = evidence(&uri);
        value.referer = Header::Text("http://evil.test/");
        assert!(endpoint.admit(value).is_none());
        let mut value = evidence(&uri);
        value.site = Header::Absent;
        value.referer = Header::Text("http://127.0.0.1:8188/");
        assert!(endpoint.admit(value).is_some());
        let mut value = evidence(&uri);
        value.site = Header::Absent;
        assert!(endpoint.admit(value).is_none());
        Ok(())
    }
    #[test]
    fn opaque_capability_binding_query_and_request_shape_are_closed() -> Result {
        let endpoint = endpoint()?;
        let uri = uri(&endpoint)?;
        assert!(endpoint.path().starts_with(RESERVED_PATH));
        assert!(endpoint.filter().ends_with('*'));
        assert!(endpoint.matches_family(&uri));
        for changed in [
            uri.replace("07070707", "08080808"),
            format!("{uri}&binding=x"),
            format!("{uri}&extra=x"),
            format!("{uri}#fragment"),
            uri.replace("binding=", "binding=%GG"),
        ] {
            assert!(endpoint.admit(evidence(&changed)).is_none());
        }
        let mut value = evidence(&uri);
        value.method = "POST";
        assert!(endpoint.admit(value).is_none());
        let mut value = evidence(&uri);
        value.resource = false;
        assert!(endpoint.admit(value).is_none());
        let mut value = evidence(&uri);
        value.range = Header::Text("bytes=1-2,4-5");
        assert!(endpoint.admit(value).is_none());
        let mut value = evidence(&uri);
        value.method = "HEAD";
        value.range = Header::Text("bytes=1-2");
        let request = endpoint.admit(value).ok_or("HEAD refused")?;
        assert!(request.head);
        assert_eq!(request.range, Some(SessionMediaRange::From { start: 1, end: Some(2) }));
        Ok(())
    }
}
