// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 Artificial Sweetener and contributors
//! Route the optional retained-media family through the existing sole resource callback owner.
use super::*;
use crate::main_session_media::route::{Evidence, Header, MAX_URI_BYTES};
use windows::core::PCWSTR;
enum Observed<const N: usize> {
    Absent,
    Text(Ascii<N>),
    Unreadable,
}
impl<const N: usize> Observed<N> {
    fn header(&self) -> Header<'_> {
        match self {
            Self::Absent => Header::Absent,
            Self::Text(value) => Header::Text(value.as_str()),
            Self::Unreadable => Header::Unreadable,
        }
    }
}
pub(super) struct Resource<'a> {
    pub(super) core: &'a ICoreWebView2,
    pub(super) environment: &'a ICoreWebView2Environment,
    pub(super) args: &'a ICoreWebView2WebResourceRequestedEventArgs,
    pub(super) uri: &'a str,
    pub(super) method: Option<&'a str>,
    pub(super) headers: &'a ICoreWebView2HttpRequestHeaders,
    pub(super) context: COREWEBVIEW2_WEB_RESOURCE_CONTEXT,
}
pub(super) fn serve(
    media: &super::super::session_media::Media,
    resource: Resource<'_>,
) -> Result<Answer, DesktopError> {
    let Resource { core, environment, args, uri, method, headers, context } = resource;
    let Some(method) = method else { return response::missing(environment).map(Answer::Response) };
    // SAFETY: All native getters run on the actual core's UI apartment before borrowing a media registry.
    let source = native_text(|output| unsafe { core.Source(output) })?;
    let Some(source) = ascii_bounded::<MAX_URI_BYTES>(&source) else {
        return response::missing(environment).map(Answer::Response);
    };
    let origin = header::<512>(headers, w!("Origin"))?;
    let referer = header::<MAX_URI_BYTES>(headers, w!("Referer"))?;
    let site = header::<64>(headers, w!("Sec-Fetch-Site"))?;
    let range = header::<64>(headers, w!("Range"))?;
    let resource = matches!(
        context,
        COREWEBVIEW2_WEB_RESOURCE_CONTEXT_IMAGE
            | COREWEBVIEW2_WEB_RESOURCE_CONTEXT_MEDIA
            | COREWEBVIEW2_WEB_RESOURCE_CONTEXT_XML_HTTP_REQUEST
            | COREWEBVIEW2_WEB_RESOURCE_CONTEXT_FETCH
    );
    let Some((requested, claim)) = media.requested(Evidence {
        uri,
        source: source.as_str(),
        method,
        resource,
        origin: origin.header(),
        referer: referer.header(),
        site: site.header(),
        range: range.header(),
    }) else {
        return response::missing(environment).map(Answer::Response);
    };
    if media.defer(requested, claim, args, environment, Instant::now())? {
        Ok(Answer::Deferred)
    } else {
        response::missing(environment).map(Answer::Response)
    }
}
fn header<const N: usize>(
    headers: &ICoreWebView2HttpRequestHeaders,
    name: PCWSTR,
) -> Result<Observed<N>, DesktopError> {
    let mut present = BOOL(0);
    // SAFETY: The live native headers write one scalar before any authored registry borrow.
    unsafe { headers.Contains(name, &mut present) }
        .map_err(|cause| DesktopError::new("session_media_header", cause))?;
    if !present.as_bool() {
        return Ok(Observed::Absent);
    }
    // SAFETY: `native_text` adopts the returned CoTaskMem output before interpreting HRESULT.
    let value = native_text(|output| unsafe { headers.GetHeader(name, output) })?;
    Ok(match ascii_bounded::<N>(&value) {
        Some(value) => Observed::Text(value),
        None => Observed::Unreadable,
    })
}
